//! Operator reset appends a cursor fact without mutating previous commits or run keys.

use serde_json::json;
use uuid::Uuid;

use crate::sensors::_helpers::facts::commit_cursor;
use crate::sensors::models::SensorCursor;
use crate::store::errors::ControlError;
use crate::store::main::append_control_event::append_control_event;
use crate::store::models::{ControlEvent, ControlTransaction};

pub(crate) async fn reset_sensor_cursor(
    transaction: ControlTransaction,
    (trigger_id, request_id): (Uuid, Uuid),
) -> Result<ControlTransaction, ControlError> {
    let cursor = SensorCursor::default();
    let event_id = Uuid::new_v5(&request_id, b"sensor-cursor-reset");
    let occurred_at = transaction.now;
    let (transaction, appended) = append_control_event(
        transaction,
        ControlEvent {
            event_id,
            invocation_id: format!("sensor/{trigger_id}"),
            event_type: "sensor_cursor_reset",
            producer: "scheduler",
            occurred_at,
            run_id: None,
            payload: json!({"trigger_id": trigger_id, "cursor": cursor}),
        },
    )
    .await?;
    if appended.conflict.is_some() {
        return Err(ControlError::Invalid(
            "sensor cursor reset conflicts with a durable fact".into(),
        ));
    }
    let storage_id = appended
        .storage_id
        .ok_or_else(|| ControlError::Invalid("sensor cursor reset has no storage cursor".into()))?;
    commit_cursor(transaction, (trigger_id, event_id, storage_id), &cursor).await
}
