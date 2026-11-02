//! Publish a new pinned execution and its operator audit atomically, including retry identity.

use serde_json::json;
use uuid::Uuid;

use crate::credentials::main::append_audit::append_audit;
use crate::credentials::models::{Action, AuditContext};
use crate::runs::_helpers::requests::permissions::authorize_execution;
use crate::runs::_helpers::requests::publication::{publish_request, request_retry};
use crate::runs::_helpers::requests::rerun::frozen_rerun;
use crate::runs::models::{RerunRequest, RunRequest};
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

/// No credentials means a trusted direct-store owner, never an HTTP-supplied role.
pub(crate) async fn request_rerun_in(
    mut transaction: ControlTransaction,
    (context, credentials): (&AuditContext, Option<(&str, &str)>),
    request: &RerunRequest,
) -> Result<(ControlTransaction, bool), ControlError> {
    let mut context = context.clone();
    let mut prepared = None;
    if let Some(credentials) = credentials {
        let rows;
        (transaction, rows) = query(
            transaction,
            "SELECT request_json FROM sqb_runs WHERE run_id=$1",
            vec![Parameter::Id(request.run_id)],
        )
        .await?;
        let frozen: RunRequest;
        if let Some(row) = rows.into_iter().next() {
            frozen = serde_json::from_value(row["request_json"].clone())
                .map_err(|error| ControlError::Invalid(error.to_string()))?;
            (transaction, context) = authorize_execution(
                transaction,
                (&context, credentials),
                (&frozen, Action::RunRerun),
            )
            .await?;
        } else {
            (transaction, (context, frozen)) =
                frozen_rerun(transaction, (&context, Some(credentials)), request).await?;
        }
        prepared = Some(frozen);
    }
    if context.actor.trim().is_empty()
        || context.granting_role.trim().is_empty()
        || request.reason.trim().is_empty()
    {
        return Err(ControlError::Invalid(
            "rerun needs an actor, granting role and reason".into(),
        ));
    }
    let event_id = Uuid::new_v5(&request.run_id, b"rerun-request");
    let (transaction, retry) = request_retry(
        transaction,
        &context,
        (request.run_id, event_id, &json!(request)),
    )
    .await?;
    if retry {
        return Ok((transaction, false));
    }
    let (transaction, frozen) = if let Some(frozen) = prepared {
        (transaction, frozen)
    } else {
        let (transaction, (_, frozen)) =
            frozen_rerun(transaction, (&context, None), request).await?;
        (transaction, frozen)
    };
    let (transaction, _) = publish_request(transaction, &frozen).await?;
    let transaction = append_audit(
        transaction,
        &context,
        (event_id, "run.rerun_requested"),
        (
            json!({"location_id": frozen.location_id, "target": frozen.target, "job_key": frozen.job_key, "run_id": request.run_id}),
            json!({"source_run_id": request.source_run_id}),
            json!({"request": request, "frozen": frozen}),
        ),
    ).await?;
    Ok((transaction, true))
}
