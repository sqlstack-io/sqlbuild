//! Heartbeats update only liveness; recovery is a durable transition in that same transaction.

use serde_json::json;
use uuid::Uuid;

use crate::runs::_helpers::lifecycle::events::append_run_event;
use crate::runs::models::{RunChange, RunChangeRequest};
use crate::store::errors::ControlError;
use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::main::query::query;
use crate::store::models::{ControlStore, Parameter, TransactionMode};

/// The worker identity is taken from its authenticated run token. Return whether cancellation is pending.
pub async fn heartbeat_run(
    store: &ControlStore,
    (run_id, claim_id): (Uuid, Uuid),
) -> Result<bool, ControlError> {
    let transaction = begin(store, TransactionMode::Write).await?;
    let now = transaction.now;
    let (transaction, rows) = query(transaction,
        "UPDATE sqb_run_liveness SET last_heartbeat_at=$1 WHERE run_id=$2 AND worker_instance=$3 RETURNING run_id",
        vec![Parameter::Time(now), Parameter::Id(run_id), Parameter::Id(claim_id)]).await?;
    if rows.is_empty() {
        return Err(ControlError::Invalid(
            "heartbeat does not belong to an active run claim".into(),
        ));
    }
    let (mut transaction, rows) = query(
        transaction,
        "SELECT status, last_event_storage_id, cancel_requested_at FROM sqb_runs WHERE run_id=$1",
        vec![Parameter::Id(run_id)],
    )
    .await?;
    let row = rows
        .first()
        .ok_or_else(|| ControlError::Invalid("claimed run does not exist".into()))?;
    let cancelled = row
        .get("cancel_requested_at")
        .is_some_and(|value| !value.is_null());
    if row.get("status").and_then(serde_json::Value::as_str) == Some("unresponsive") {
        let previous = row
            .get("last_event_storage_id")
            .and_then(serde_json::Value::as_i64)
            .ok_or_else(|| {
                ControlError::Invalid("run recovery is missing its preceding event".into())
            })?;
        let request = RunChangeRequest {
            event_id: Uuid::new_v5(&claim_id, format!("recovery/{previous}").as_bytes()),
            run_id,
            claim_id: Some(claim_id),
            change: RunChange::Recovered,
        };
        let cursor;
        (transaction, cursor) = append_run_event(
            transaction,
            (run_id, request.event_id),
            request.change.event_type(),
            json!({"change": request}),
        )
        .await?;
        (transaction, _) = query(
            transaction,
            "UPDATE sqb_runs SET status='running', last_event_storage_id=$1 WHERE run_id=$2",
            vec![Parameter::Integer(cursor), Parameter::Id(run_id)],
        )
        .await?;
    }
    commit(transaction).await?;
    Ok(cancelled)
}
