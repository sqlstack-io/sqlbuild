//! Retry-safe facts, lifecycle projection and terminal claim release form one transaction.

use serde_json::json;

use crate::runs::_helpers::changes::{validate_authority, validate_change};
use crate::runs::_helpers::events::append_run_event;
use crate::runs::_helpers::projection::project_change;
use crate::runs::models::{ChangingRun, RunAuthority, RunChange, RunChangeRequest, RunOutcome};
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
        if previous.get("payload") != Some(&body)
            || previous
                .get("invocation_id")
                .and_then(serde_json::Value::as_str)
                != Some(format!("scheduler/{}", request.run_id).as_str())
        {
            return Err(ControlError::Invalid(
                "run change event ID already has different content".into(),
            ));
        }
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
    let (transaction, mut cursor) = append_run_event(
        transaction,
        (request.run_id, request.event_id),
        request.change.event_type(),
        body,
    )
    .await?;
    let mut transaction = project_change(transaction, request, &state).await?;
    if matches!(request.change, RunChange::CancelRequested { .. })
        && state.worker_instance.is_none()
    {
        let terminal = RunChangeRequest {
            event_id: uuid::Uuid::new_v5(&request.event_id, b"cancelled-before-launch"),
            run_id: request.run_id,
            claim_id: None,
            change: RunChange::Finished {
                outcome: RunOutcome::Cancelled,
                reason: "cancelled before launch".into(),
                exit_code: None,
            },
        };
        (transaction, cursor) = append_run_event(
            transaction,
            (terminal.run_id, terminal.event_id),
            terminal.change.event_type(),
            json!({"change": terminal}),
        )
        .await?;
        transaction = project_change(transaction, &terminal, &state).await?;
    }
    let (mut transaction, _) = query(
        transaction,
        "UPDATE sqb_runs SET last_event_storage_id=$1 WHERE run_id=$2",
        vec![Parameter::Integer(cursor), Parameter::Id(request.run_id)],
    )
    .await?;
    if let RunAuthority::Scheduler(lease) = authority {
        transaction = check_lease(transaction, lease).await?;
    }
    commit(transaction).await?;
    Ok(true)
}
