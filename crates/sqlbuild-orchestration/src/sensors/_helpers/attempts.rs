//! Indexed reads recover attempts and schedule projections without mutable phase transitions.

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::sensors::models::{DueSensor, SensorEvaluationFact};
use crate::store::errors::ControlError;
use crate::store::main::decode_row::decode_row;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

pub(crate) async fn read_sensor(
    transaction: ControlTransaction,
    trigger_id: Uuid,
) -> Result<(ControlTransaction, Option<DueSensor>), ControlError> {
    let (transaction, rows) = query(transaction,
        "SELECT t.trigger_id,t.location_id,t.target,t.snapshot_id,t.definition_json,s.status \
         FROM sqb_triggers t JOIN sqb_trigger_state s ON s.trigger_id=t.trigger_id \
         JOIN sqb_locations l ON l.location_id=t.location_id WHERE t.trigger_id=$1 AND t.kind='sensor' \
         AND t.removed_in_snapshot_id IS NULL AND l.archived_at IS NULL",
        vec![Parameter::Id(trigger_id)]).await?;
    Ok((
        transaction,
        rows.into_iter().next().map(decode_row).transpose()?,
    ))
}

pub(crate) async fn last_attempt(
    transaction: ControlTransaction,
    trigger_id: Uuid,
) -> Result<(ControlTransaction, Option<(SensorEvaluationFact, bool)>), ControlError> {
    let (transaction, rows) = query(transaction,
        "SELECT e.fact_json, CASE WHEN EXISTS (SELECT 1 FROM sqb_sensor_evaluation_events terminal \
         WHERE terminal.evaluation_id=e.evaluation_id AND terminal.phase<>'requested') THEN 1 ELSE 0 END AS terminal \
         FROM sqb_sensor_evaluation_events e WHERE e.trigger_id=$1 AND e.phase='requested' ORDER BY e.storage_id DESC LIMIT 1",
        vec![Parameter::Id(trigger_id)]).await?;
    let last = rows
        .first()
        .map(|row| {
            let fact = serde_json::from_value(row["fact_json"].clone())
                .map_err(|error| ControlError::Invalid(error.to_string()))?;
            Ok::<_, ControlError>((fact, row["terminal"].as_i64() == Some(1)))
        })
        .transpose()?;
    Ok((transaction, last))
}

pub(crate) async fn defer_sensor(
    transaction: ControlTransaction,
    trigger_id: Uuid,
    next: DateTime<Utc>,
) -> Result<ControlTransaction, ControlError> {
    let now = transaction.now;
    let (transaction, _) = query(
        transaction,
        "UPDATE sqb_trigger_state SET next_due_at=$1,updated_at=$2 WHERE trigger_id=$3",
        vec![
            Parameter::Time(next),
            Parameter::Time(now),
            Parameter::Id(trigger_id),
        ],
    )
    .await?;
    Ok(transaction)
}
