//! A sensor is failing while its newest finished ticks failed or timed out; abandoned ticks don't count either way.

use uuid::Uuid;

use crate::sensors::constants::FAILING_TICKS_READ;
use crate::sensors::models::{
    SensorEvaluationFact, SensorEvaluationOutcome, SensorEvent, SensorFailing,
};
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

/// Since when the sensor's ticks have failed, and the event the newest failed one was called with.
pub(crate) async fn sensor_failing(
    transaction: ControlTransaction,
    trigger_id: Uuid,
) -> Result<(ControlTransaction, Option<SensorFailing>), ControlError> {
    let (transaction, rows) = query(
        transaction,
        &format!(
            "SELECT fact_json FROM sqb_sensor_evaluation_events WHERE trigger_id=$1 \
             AND phase IN ('completed','failed','timed_out') ORDER BY occurred_at DESC, event_id DESC \
             LIMIT {FAILING_TICKS_READ}"
        ),
        vec![Parameter::Id(trigger_id)],
    )
    .await?;
    let mut failing: Option<SensorFailing> = None;
    for row in rows {
        let fact: SensorEvaluationFact = serde_json::from_value(row["fact_json"].clone())
            .map_err(|error| ControlError::Invalid(error.to_string()))?;
        if matches!(fact.outcome, SensorEvaluationOutcome::Completed { .. }) {
            break;
        }
        let request = &fact.request;
        let streak = failing.get_or_insert_with(|| SensorFailing {
            since: request.requested_at,
            failed_ticks: 0,
            event_id: request.event.as_ref().map(|event| event.id().to_owned()),
            event_summary: request.event.as_ref().map(SensorEvent::summary),
        });
        streak.since = request.requested_at;
        streak.failed_ticks += 1;
    }
    Ok((transaction, failing))
}
