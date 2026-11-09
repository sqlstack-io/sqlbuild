//! Manual job freezing, idempotence, authorization and publication share one transaction.

use serde_json::json;
use uuid::Uuid;

use crate::authorization::main::authorize_person::authorize_person;
use crate::authorization::main::refuse_unknown::refuse_unknown;
use crate::credentials::main::audit::append_audit::append_audit;
use crate::credentials::models::{Action, AuditContext, DefaultRole, PermissionScope};
use crate::runs::_helpers::requests::permissions::authorize_execution;
use crate::runs::_helpers::requests::publication::{
    publish_request, record_authored, request_retry,
};
use crate::runs::constants::MANUAL_JOB_AUDIT_TAG;
use crate::runs::main::frozen_job_request::frozen_job_request;
use crate::runs::models::{JobRunRequest, RunWindow};
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

pub(crate) async fn request_job_run_in(
    mut transaction: ControlTransaction,
    (context, credentials): (&AuditContext, Option<Uuid>),
    request: &JobRunRequest,
) -> Result<(ControlTransaction, bool), ControlError> {
    let mut context = context.clone();
    let scope = PermissionScope {
        action: Action::JobRun,
        location_id: Some(request.location_id),
        target: Some(request.target.clone()),
        nodes: Default::default(),
    };
    if let Some(credentials) = credentials {
        (transaction, context) = authorize_person(
            transaction,
            (&context, credentials),
            &scope,
            DefaultRole::None,
        )
        .await?;
    }
    if context.actor.trim().is_empty()
        || context.granting_role.trim().is_empty()
        || request.target.trim().is_empty()
        || request.job_key.trim().is_empty()
        || request.reason.trim().is_empty()
    {
        return Err(ControlError::Invalid(
            "job run needs an actor, granting role, target, job and reason".into(),
        ));
    }
    if request.window.as_ref().is_some_and(|window| match window {
        RunWindow::Timestamp { start, end } => start > end,
        RunWindow::Integer { start, end } => start > end,
    }) {
        return Err(ControlError::Invalid(
            "backfill window start must not follow end".into(),
        ));
    }
    if request.full_refresh && request.window.is_some() {
        return Err(ControlError::Invalid(
            "a backfill window and a full refresh cannot be combined".into(),
        ));
    }
    let event_id = Uuid::new_v5(&request.run_id, MANUAL_JOB_AUDIT_TAG);
    let (transaction, retry) =
        request_retry(transaction, &context, (request.run_id, &json!(request))).await?;
    if retry {
        return Ok((transaction, false));
    }
    let transaction = match credentials {
        Some(credentials) => known_job(transaction, (&context, credentials), request).await?,
        None => transaction,
    };
    let (mut transaction, frozen) = frozen_job_request(transaction, &context, request).await?;
    if let Some(credentials) = credentials {
        (transaction, context) = authorize_execution(
            transaction,
            (&context, credentials),
            (&frozen, Action::JobRun),
        )
        .await?;
    }
    let (transaction, _) = publish_request(transaction, &frozen).await?;
    let transaction =
        record_authored(transaction, &context, (request.run_id, &json!(request))).await?;
    let transaction = append_audit(transaction, &context, (event_id, "job.run_requested"),
        (json!({"location_id":request.location_id,"target":request.target,"job_key":request.job_key,"run_id":request.run_id}),
         json!(null), json!({"request":request,"frozen":frozen}))).await?;
    Ok((transaction, true))
}

/// A person's unknown job is refused like a forbidden one unless they may run every node of the target.
async fn known_job(
    transaction: ControlTransaction,
    credentials: (&AuditContext, Uuid),
    request: &JobRunRequest,
) -> Result<ControlTransaction, ControlError> {
    let (transaction, jobs) = query(transaction,
        "SELECT j.job_key FROM sqb_jobs j JOIN sqb_current_snapshots c ON c.location_id=j.location_id \
         AND c.target=j.target AND c.snapshot_id=j.snapshot_id WHERE j.location_id=$1 AND j.target=$2 AND j.job_key=$3",
        vec![Parameter::Id(request.location_id), Parameter::Text(request.target.clone()), Parameter::Text(request.job_key.clone())]).await?;
    if jobs.is_empty() {
        return refuse_unknown(
            transaction,
            credentials,
            (Action::JobRun, Some((request.location_id, &request.target))),
            ControlError::Invalid("published job not found in this location and target".into()),
        )
        .await;
    }
    Ok(transaction)
}
