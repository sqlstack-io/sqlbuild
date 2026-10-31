//! Retry-safe facts, lifecycle projection and terminal claim release form one transaction.

use serde_json::json;

use crate::credentials::main::append_audit::append_audit;
use crate::runs::_helpers::lifecycle::changes::{
    validate_authority, validate_change, validate_retry,
};
use crate::runs::_helpers::lifecycle::publish_change::publish_change;
use crate::runs::models::{ChangingRun, RunAuthority, RunChange, RunChangeRequest};
use crate::scheduler::main::check_lease::check_lease;
use crate::store::errors::ControlError;
use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::main::decode_row::decode_row;
use crate::store::main::query::query;
use crate::store::models::{ControlStore, Parameter, TransactionMode};

/// Use authenticated authority and record unclaimed cancellation as terminal in the same transaction.
pub async fn change_run(
    store: &ControlStore,
    authority: RunAuthority<'_>,
    request: &RunChangeRequest,
) -> Result<bool, ControlError> {
    validate_authority(authority, request)?;
    let mut transaction = begin(store, TransactionMode::Write).await?;
    if let RunAuthority::Scheduler(lease) = authority {
        transaction = check_lease(transaction, lease).await?;
    }
    let body = json!({"change": request});
    let (transaction, previous) = query(
        transaction,
        "SELECT invocation_id, payload FROM recall_events WHERE event_id=$1 LIMIT 1",
        vec![Parameter::Text(request.event_id.to_string())],
    )
    .await?;
    if let Some(previous) = previous.first() {
        validate_retry(previous, request, &body)?;
        commit(transaction).await?;
        return Ok(false);
    }
    let (transaction, rows) = query(
        transaction,
        "SELECT r.status, r.cancel_requested_at, r.node_counts_json, l.worker_instance FROM sqb_runs r \
         LEFT JOIN sqb_run_liveness l ON l.run_id=r.run_id WHERE r.run_id=$1",
        vec![Parameter::Id(request.run_id)],
    )
    .await?;
    let state: ChangingRun = decode_row(
        rows.into_iter()
            .next()
            .ok_or_else(|| ControlError::Invalid("run does not exist".into()))?,
    )?;
    validate_change(&state, request)?;
    let mut transaction = publish_change(transaction, request, &state).await?;
    if let (RunAuthority::Operator(context), RunChange::CancelRequested { reason, .. }) =
        (authority, &request.change)
    {
        let mut rows;
        (transaction, rows) = query(
            transaction,
            "SELECT location_id, target, status, cancel_requested_at FROM sqb_runs WHERE run_id=$1",
            vec![Parameter::Id(request.run_id)],
        )
        .await?;
        let after = rows.remove(0);
        transaction = append_audit(
            transaction,
            context,
            (request.event_id, "run.cancel_requested"),
            (
                json!({"location_id": after["location_id"], "target": after["target"], "run_id": request.run_id}),
                json!({"status": state.status, "cancel_requested_at": state.cancel_requested_at}),
                json!({"status": after["status"], "cancel_requested_at": after["cancel_requested_at"], "reason": reason}),
            ),
        )
        .await?;
    }
    if let RunAuthority::Scheduler(lease) = authority {
        transaction = check_lease(transaction, lease).await?;
    }
    commit(transaction).await?;
    Ok(true)
}
