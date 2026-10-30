//! All orchestration transitions share Recall's invocation sequence and transaction.

use chrono::SecondsFormat;
use serde_json::{Map, Value, json};
use sqlbuild_recall::records::main::event_object_row::event_object_row;
use sqlbuild_recall::records::models::RowEnvelope;
use uuid::Uuid;

use crate::store::errors::ControlError;
use crate::store::main::append_recall::append_recall;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

pub(crate) async fn append_run_event(
    transaction: ControlTransaction,
    (run_id, event_id): (Uuid, Uuid),
    kind: &str,
    payload: Value,
) -> Result<(ControlTransaction, i64), ControlError> {
    let invocation_id = format!("scheduler/{run_id}");
    let timestamp = transaction.now.to_rfc3339_opts(SecondsFormat::Micros, true);
    let (transaction, rows) = query(transaction,
        "SELECT invocation_sequence AS last_sequence, invocation_started_at AS started_at FROM recall_events \
         WHERE invocation_id=$1 ORDER BY invocation_sequence DESC LIMIT 1",
        vec![Parameter::Text(invocation_id.clone())]).await?;
    let previous = rows.first();
    let sequence = previous
        .and_then(|row| row.get("last_sequence"))
        .and_then(Value::as_i64)
        .map_or(Some(0), |value| value.checked_add(1))
        .ok_or_else(|| ControlError::Invalid("run event sequence is exhausted".into()))?;
    let envelope = RowEnvelope {
        invocation_id,
        invocation_started_at: previous
            .and_then(|row| row.get("started_at"))
            .and_then(Value::as_str)
            .unwrap_or(&timestamp)
            .into(),
        producer_version: env!("CARGO_PKG_VERSION").into(),
    };
    let event = Map::from_iter([
        ("event_id".into(), json!(event_id)),
        ("event_type".into(), json!(kind)),
        ("schema_version".into(), json!(1)),
        ("occurred_at".into(), json!(timestamp)),
        ("producer".into(), json!("scheduler")),
        ("invocation_sequence".into(), json!(sequence)),
        ("run_id".into(), json!(run_id)),
        ("payload".into(), payload),
    ]);
    let row = event_object_row(&envelope, &event)
        .map_err(|error| ControlError::Invalid(error.to_string()))?;
    let (transaction, outcome) = append_recall(transaction, &row).await?;
    if outcome.conflict.is_some() {
        return Err(ControlError::Invalid(
            "run event conflicts with a durable fact".into(),
        ));
    }
    let cursor = outcome
        .storage_id
        .ok_or_else(|| ControlError::Invalid("run event has no storage cursor".into()))?;
    Ok((transaction, cursor))
}
