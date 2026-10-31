//! Retry-safe facts, lifecycle projection and terminal claim release form one transaction.

use serde_json::json;

use crate::runs::_helpers::changes::{validate_authority, validate_change, validate_retry};
use crate::runs::_helpers::publish_change::publish_change;
use crate::runs::models::{ChangingRun, RunAuthority, RunChangeRequest};
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
    if let RunAuthority::Scheduler(lease) = authority {
        transaction = check_lease(transaction, lease).await?;
    }
    commit(transaction).await?;
    Ok(true)
}
