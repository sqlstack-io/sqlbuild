//! Indexed reads recover attempts and schedule projections without mutable phase transitions.

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::sensors::models::{DueSensor, SensorEvaluationFact};
use crate::sensors::types::LastAttempt;
use crate::snapshots::main::reads::decode_published_row::decode_published_row;
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};
use crate::store::types::Row;

/// The due sensor; a published definition that no longer decodes is `ControlError::UnreadableSnapshot`, which skips only this sensor.
pub(crate) async fn read_sensor(
    transaction: ControlTransaction,
    trigger_id: Uuid,
) -> Result<(ControlTransaction, Option<DueSensor>), ControlError> {
    let (transaction, rows) = query(transaction,
        "SELECT t.trigger_id,t.name,t.location_id,t.target,t.snapshot_id,t.definition_json,s.status \
         FROM sqb_triggers t JOIN sqb_trigger_state s ON s.trigger_id=t.trigger_id \
         JOIN sqb_locations l ON l.location_id=t.location_id WHERE t.trigger_id=$1 AND t.kind='sensor' \
         AND t.removed_in_snapshot_id IS NULL AND l.archived_at IS NULL",
        vec![Parameter::Id(trigger_id)]).await?;
    let sensor = rows
        .into_iter()
        .next()
        .map(decode_published_row)
        .transpose()?;
    Ok((transaction, sensor))
}

/// The newest requested evaluation and its terminal phase (`None` while it is in flight); `Some(Err(detail))` when it was recorded by an earlier release in a form that no longer decodes. Facts are append-only, so a caller treats that as no previous attempt rather than failing until the next one.
pub(crate) async fn last_attempt(
    transaction: ControlTransaction,
    trigger_id: Uuid,
) -> Result<(ControlTransaction, Option<LastAttempt>), ControlError> {
    let (transaction, rows) = query(transaction,
        "SELECT e.fact_json, (SELECT terminal.phase FROM sqb_sensor_evaluation_events terminal \
         WHERE terminal.evaluation_id=e.evaluation_id AND terminal.phase<>'requested' LIMIT 1) AS terminal \
         FROM sqb_sensor_evaluation_events e WHERE e.trigger_id=$1 AND e.phase='requested' \
         ORDER BY e.occurred_at DESC, e.evaluation_id DESC LIMIT 1",
        vec![Parameter::Id(trigger_id)]).await?;
    let last: Option<LastAttempt> = rows.first().map(decoded_attempt);
    Ok((transaction, last))
}

/// One requested evaluation row as a fact and its terminal phase, or why its fact no longer decodes.
fn decoded_attempt(row: &Row) -> LastAttempt {
    match serde_json::from_value::<SensorEvaluationFact>(row["fact_json"].clone()) {
        Ok(fact) => Ok((fact, row["terminal"].as_str().map(str::to_owned))),
        Err(error) => Err(error.to_string()),
    }
}

pub(crate) async fn defer_sensor(
    transaction: ControlTransaction,
    trigger_id: Uuid,
    next: DateTime<Utc>,
) -> Result<ControlTransaction, ControlError> {
    let (transaction, _) = query(
        transaction,
        "UPDATE sqb_trigger_state SET next_due_at=$1 WHERE trigger_id=$2 \
         AND next_due_at IS DISTINCT FROM $1",
        vec![Parameter::Time(next), Parameter::Id(trigger_id)],
    )
    .await?;
    Ok(transaction)
}
