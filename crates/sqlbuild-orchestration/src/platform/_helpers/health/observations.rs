//! Replicas share a deterministic observation key, so each alert incident is recorded once.

use chrono::{DateTime, DurationRound, TimeDelta, Utc};
use serde_json::json;
use uuid::Uuid;

use crate::platform::constants::OBSERVATION_WINDOW_SECONDS;
use crate::platform::models::PlatformEvent;
use crate::store::errors::ControlError;
use crate::store::main::append_control_event::append_control_event;
use crate::store::main::query::query;
use crate::store::models::{ControlEvent, ControlTransaction, Parameter};

/// A heartbeat is one fact per 30-second window; an alert is one fact per payload and `incident`.
pub(crate) async fn observe(
    transaction: ControlTransaction,
    installation_id: Uuid,
    (event, incident): (PlatformEvent, Option<DateTime<Utc>>),
) -> Result<ControlTransaction, ControlError> {
    observe_episode(transaction, installation_id, (event, None, incident)).await
}

/// `observe`, with `episode` in place of the payload as the alert's identity, so the episode's first payload stays.
pub(crate) async fn observe_episode(
    transaction: ControlTransaction,
    installation_id: Uuid,
    (event, episode, incident): (PlatformEvent, Option<PlatformEvent>, Option<DateTime<Utc>>),
) -> Result<ControlTransaction, ControlError> {
    let occurred_at = transaction
        .now
        .duration_trunc(TimeDelta::seconds(OBSERVATION_WINDOW_SECONDS))
        .map_err(|error| ControlError::Invalid(error.to_string()))?;
    let occurrence = match event {
        PlatformEvent::Heartbeat { .. } => Some(occurred_at),
        _ => incident,
    };
    let payload = json!({"platform": event});
    let identity = serde_json::to_vec(&(occurrence, episode.as_ref().unwrap_or(&event)))
        .map_err(|error| ControlError::Invalid(error.to_string()))?;
    let event_id = Uuid::new_v5(&installation_id, &identity);
    let (transaction, previous) = query(
        transaction,
        "SELECT 1 AS present FROM recall_events WHERE event_type=$1 AND event_id=$2 LIMIT 1",
        vec![
            Parameter::Text(event.event_type().into()),
            Parameter::Text(event_id.to_string()),
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
