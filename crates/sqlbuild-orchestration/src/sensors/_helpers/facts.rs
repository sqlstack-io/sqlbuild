//! Sensor evaluation phases are recorded once; the latest cursor lives on the trigger's state.

use std::collections::BTreeSet;

use serde_json::json;
use sqlbuild_recall::records::main::sha256_hex::sha256_hex;
use uuid::Uuid;

use crate::sensors::models::{
    SensorCursor, SensorDefinition, SensorEvaluationFact, SensorEvaluationOutcome,
    SensorEvaluationRequest,
};
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{Change, ControlTransaction, Parameter};

/// Evaluations record the fingerprint of the definition they ran, so a later definition shows as changed; where it is written is not part of it.
pub(crate) fn definition_fingerprint(
    definition: &SensorDefinition,
) -> Result<String, ControlError> {
    let behaviour: SensorDefinition = SensorDefinition {
        targets: BTreeSet::new(),
        declared_at: None,
        ..definition.clone()
    };
    Ok(sha256_hex(&serde_json::to_vec(&behaviour).map_err(
        |error| ControlError::Invalid(error.to_string()),
    )?))
}

pub(crate) async fn append_evaluation(
    transaction: ControlTransaction,
    request: &SensorEvaluationRequest,
    outcome: SensorEvaluationOutcome,
) -> Result<ControlTransaction, ControlError> {
    let phase = outcome.phase();
    let event_id = Uuid::new_v5(&request.evaluation_id, phase.as_bytes());
    let occurred_at = transaction.now;
    let fact = SensorEvaluationFact {
        request: request.clone(),
        outcome,
    };
    let (mut transaction, _) = query(transaction,
        "INSERT INTO sqb_sensor_evaluation_events (event_id,evaluation_id,trigger_id,phase,occurred_at,fact_json) \
         VALUES ($1,$2,$3,$4,$5,$6)",
        vec![Parameter::Id(event_id), Parameter::Id(request.evaluation_id), Parameter::Id(request.trigger_id),
            Parameter::Text(phase.into()), Parameter::Time(occurred_at), Parameter::Json(json!(fact))]).await?;
    transaction
        .changes
        .insert(Change::Trigger(request.trigger_id));
    Ok(transaction)
}

pub(crate) async fn commit_cursor(
    transaction: ControlTransaction,
    trigger_id: Uuid,
    cursor: &SensorCursor,
) -> Result<ControlTransaction, ControlError> {
    let (transaction, _) = query(
        transaction,
        "UPDATE sqb_trigger_state SET cursor_json=$1 WHERE trigger_id=$2",
        vec![Parameter::Json(json!(cursor)), Parameter::Id(trigger_id)],
    )
    .await?;
    Ok(transaction)
}
