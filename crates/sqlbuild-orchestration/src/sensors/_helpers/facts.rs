//! Attempts and cursor commits use Recall's serialized commit-order sequence.

use serde_json::json;
use uuid::Uuid;

use crate::sensors::models::{
    SensorCursor, SensorEvaluationFact, SensorEvaluationOutcome, SensorEvaluationRequest,
};
use crate::store::errors::ControlError;
use crate::store::main::append_control_event::append_control_event;
use crate::store::main::query::query;
use crate::store::models::{ControlEvent, ControlTransaction, Parameter};

pub(crate) async fn append_evaluation(
    transaction: ControlTransaction,
    request: &SensorEvaluationRequest,
    outcome: SensorEvaluationOutcome,
) -> Result<(ControlTransaction, (Uuid, i64)), ControlError> {
    let phase = outcome.phase();
    let event_id = Uuid::new_v5(&request.evaluation_id, phase.as_bytes());
    let occurred_at = transaction.now;
    let fact = SensorEvaluationFact {
        request: request.clone(),
        outcome,
    };
    let payload = json!(fact);
    let (transaction, appended) = append_control_event(
        transaction,
        ControlEvent {
            event_id,
            invocation_id: format!("sensor/{}", request.trigger_id),
            event_type: "sensor_evaluation",
            producer: "scheduler",
            occurred_at,
            run_id: None,
            payload: payload.clone(),
        },
    )
    .await?;
    if appended.conflict.is_some() {
        return Err(ControlError::Invalid(
            "sensor evaluation conflicts with a durable fact".into(),
        ));
    }
    let storage_id = appended
        .storage_id
        .ok_or_else(|| ControlError::Invalid("sensor evaluation has no storage cursor".into()))?;
    let (transaction, _) = query(transaction,
        "INSERT INTO sqb_sensor_evaluation_events (event_id,storage_id,evaluation_id,trigger_id,phase,occurred_at,fact_json) \
         VALUES ($1,$2,$3,$4,$5,$6,$7)",
        vec![Parameter::Id(event_id), Parameter::Integer(storage_id), Parameter::Id(request.evaluation_id),
            Parameter::Id(request.trigger_id), Parameter::Text(phase.into()), Parameter::Time(occurred_at), Parameter::Json(payload)]).await?;
    Ok((transaction, (event_id, storage_id)))
}

pub(crate) async fn commit_cursor(
    transaction: ControlTransaction,
    (trigger_id, event_id, storage_id): (Uuid, Uuid, i64),
    cursor: &SensorCursor,
) -> Result<ControlTransaction, ControlError> {
    let (transaction, _) = query(transaction,
        "INSERT INTO sqb_sensor_cursor_commits (event_id,storage_id,trigger_id,cursor_json) VALUES ($1,$2,$3,$4)",
        vec![Parameter::Id(event_id), Parameter::Integer(storage_id), Parameter::Id(trigger_id), Parameter::Json(json!(cursor))]).await?;
    Ok(transaction)
}
