//! Publish a new pinned execution and its operator audit atomically, including retry identity.

use serde_json::json;
use uuid::Uuid;

use crate::credentials::main::append_audit::append_audit;
use crate::credentials::models::AuditContext;
use crate::runs::_helpers::requests::publication::{publish_request, request_retry};
use crate::runs::_helpers::requests::rerun::frozen_rerun;
use crate::runs::models::RerunRequest;
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit};
use crate::store::models::{ControlStore, TransactionMode};

/// This entry requires direct control-store ownership, just like a privileged manual job request.
pub async fn request_rerun(
    store: &ControlStore,
    context: &AuditContext,
    request: &RerunRequest,
) -> Result<bool, ControlError> {
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
        begin(store, TransactionMode::Write).await?,
        context,
        (request.run_id, event_id, &json!(request)),
    )
    .await?;
    if retry {
        commit(transaction).await?;
        return Ok(false);
    }
    let (transaction, frozen) = frozen_rerun(transaction, context, request).await?;
    let (transaction, _) = publish_request(transaction, &frozen).await?;
    let transaction = append_audit(
        transaction,
        context,
        (event_id, "run.rerun_requested"),
        (
            json!({"location_id": frozen.location_id, "target": frozen.target, "job_key": frozen.job_key, "run_id": request.run_id}),
            json!({"source_run_id": request.source_run_id}),
            json!({"request": request, "frozen": frozen}),
        ),
    ).await?;
    commit(transaction).await?;
    Ok(true)
}
