//! Direct-store owners queue published jobs without supplying or widening their write scope.

use serde_json::json;
use uuid::Uuid;

use crate::credentials::main::append_audit::append_audit;
use crate::credentials::models::AuditContext;
use crate::runs::_helpers::job_request::frozen_job_request;
use crate::runs::_helpers::request::publish_request;
use crate::runs::models::JobRunRequest;
use crate::store::errors::ControlError;
use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::main::query::query;
use crate::store::models::{ControlStore, Parameter, TransactionMode};

/// The context is a trusted control-store owner, not a role supplied by an HTTP caller.
pub async fn request_job_run(
    store: &ControlStore,
    context: &AuditContext,
    request: &JobRunRequest,
) -> Result<bool, ControlError> {
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
    let event_id = Uuid::new_v5(&request.run_id, b"manual-job-request");
    let transaction = begin(store, TransactionMode::Write).await?;
    let (transaction, previous) = query(
        transaction,
        "SELECT a.actor, a.after_json FROM sqb_runs r LEFT JOIN sqb_security_audit a ON a.event_id=$1 WHERE r.run_id=$2",
        vec![Parameter::Id(event_id), Parameter::Id(request.run_id)],
    ).await?;
    if let Some(previous) = previous.first() {
        if previous.get("actor") != Some(&json!(context.actor))
            || previous
                .get("after_json")
                .and_then(|after| after.get("request"))
                != Some(&json!(request))
        {
            return Err(ControlError::Invalid(
                "run ID already has different request content".into(),
            ));
        }
        commit(transaction).await?;
        return Ok(false);
    }
    let (transaction, frozen) = frozen_job_request(transaction, context, request).await?;
    let (transaction, _) = publish_request(transaction, &frozen).await?;
    let transaction = append_audit(
        transaction,
        context,
        (event_id, "job.run_requested"),
        (
            json!({"location_id": request.location_id, "target": request.target, "job_key": request.job_key, "run_id": request.run_id}),
            serde_json::Value::Null,
            json!({"request": request, "frozen": frozen}),
        ),
    ).await?;
    commit(transaction).await?;
    Ok(true)
}
