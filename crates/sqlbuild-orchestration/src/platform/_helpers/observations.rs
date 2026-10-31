//! Replicas share a deterministic observation key and a serialized append transaction.

use chrono::{DurationRound, TimeDelta};
use serde_json::json;
use uuid::Uuid;

use crate::platform::models::PlatformEvent;
use crate::store::errors::ControlError;
use crate::store::main::append_control_event::append_control_event;
use crate::store::main::query::query;
use crate::store::models::{ControlEvent, ControlTransaction, Parameter};

pub(crate) async fn observe(
    transaction: ControlTransaction,
    installation_id: Uuid,
    event: PlatformEvent,
) -> Result<ControlTransaction, ControlError> {
    let occurred_at = transaction
        .now
        .duration_trunc(TimeDelta::seconds(30))
        .map_err(|error| ControlError::Invalid(error.to_string()))?;
    let payload = json!({"platform": event});
    let identity = serde_json::to_vec(&(occurred_at, &event))
        .map_err(|error| ControlError::Invalid(error.to_string()))?;
    let event_id = Uuid::new_v5(&installation_id, &identity);
    let (transaction, previous) = query(
        transaction,
        "SELECT 1 AS present FROM recall_events WHERE event_id=$1 AND occurred_at=$2 LIMIT 1",
        vec![
            Parameter::Text(event_id.to_string()),
            Parameter::Time(occurred_at),
        ],
    )
    .await?;
    if !previous.is_empty() {
        return Ok(transaction);
    }
    let (transaction, outcome) = append_control_event(
        transaction,
        ControlEvent {
            event_id,
            invocation_id: format!("platform/{installation_id}/{}", occurred_at.date_naive()),
            event_type: event.event_type(),
            producer: "platform",
            occurred_at,
            run_id: None,
            payload,
        },
    )
    .await?;
    if outcome.conflict.is_some() {
        return Err(ControlError::Invalid(
            "platform observation conflicts with a durable fact".into(),
        ));
    }
    Ok(transaction)
}
