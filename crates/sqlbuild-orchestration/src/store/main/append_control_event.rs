//! Sequence scheduler and platform facts inside the caller's serialized transaction.

use chrono::SecondsFormat;
use serde_json::{Map, Value, json};
use sqlbuild_recall::records::main::event_object_row::event_object_row;
use sqlbuild_recall::records::models::RowEnvelope;

use crate::store::errors::ControlError;
use crate::store::main::append_recall::append_recall;
use crate::store::main::query::query;
use crate::store::models::{ControlEvent, ControlTransaction, Parameter, RecallAppendOutcome};

pub(crate) async fn append_control_event(
    transaction: ControlTransaction,
    event: ControlEvent<'_>,
) -> Result<(ControlTransaction, RecallAppendOutcome), ControlError> {
    let timestamp = event
        .occurred_at
        .to_rfc3339_opts(SecondsFormat::Micros, true);
    let (transaction, rows) = query(transaction,
        "SELECT invocation_sequence AS last_sequence, invocation_started_at AS started_at FROM recall_events \
         WHERE invocation_id=$1 ORDER BY invocation_sequence DESC LIMIT 1",
        vec![Parameter::Text(event.invocation_id.clone())]).await?;
    let previous = rows.first();
    let sequence = previous
        .and_then(|row| row.get("last_sequence"))
        .and_then(Value::as_i64)
        .map_or(Some(0), |value| value.checked_add(1))
        .ok_or_else(|| ControlError::Invalid("run event sequence is exhausted".into()))?;
    let envelope = RowEnvelope {
        invocation_id: event.invocation_id,
        invocation_started_at: previous
            .and_then(|row| row.get("started_at"))
            .and_then(Value::as_str)
            .unwrap_or(&timestamp)
            .into(),
        producer_version: env!("CARGO_PKG_VERSION").into(),
    };
    let object = Map::from_iter([
        ("event_id".into(), json!(event.event_id)),
        ("event_type".into(), json!(event.event_type)),
        ("schema_version".into(), json!(1)),
        ("occurred_at".into(), json!(timestamp)),
        ("producer".into(), json!(event.producer)),
        ("invocation_sequence".into(), json!(sequence)),
        ("run_id".into(), json!(event.run_id)),
        ("payload".into(), event.payload),
    ]);
    let row = event_object_row(&envelope, &object)
        .map_err(|error| ControlError::Invalid(error.to_string()))?;
    append_recall(transaction, &row).await
}
