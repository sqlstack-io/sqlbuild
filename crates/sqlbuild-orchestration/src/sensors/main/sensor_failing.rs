//! A sensor is failing from the first failed, timed-out or unanswered tick after its last good one; other abandoned ticks don't count either way.

use uuid::Uuid;

use crate::sensors::constants::FAILING_TICK_PHASES;
use crate::sensors::models::{
    AbandonReason, SensorEvaluationFact, SensorEvaluationOutcome, SensorEvent, SensorFailing,
};
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

/// Since when the sensor's ticks have failed, how many have, and the event and reason of the newest; three indexed reads however long the streak.
pub(crate) async fn sensor_failing(
    transaction: ControlTransaction,
    trigger_id: Uuid,
) -> Result<(ControlTransaction, Option<SensorFailing>), ControlError> {
    let streak = format!(
        "FROM sqb_sensor_evaluation_events WHERE trigger_id=$1 AND {FAILING_TICK_PHASES} \
         AND occurred_at > COALESCE((SELECT MAX(occurred_at) FROM sqb_sensor_evaluation_events \
         WHERE trigger_id=$1 AND phase='completed'), $2)"
    );
    let (transaction, counted) = query(
        transaction,
        &format!("SELECT COUNT(*) AS failed_ticks {streak}"),
        vec![
            Parameter::Id(trigger_id),
            Parameter::Time(chrono::DateTime::UNIX_EPOCH),
        ],
    )
    .await?;
    let failed_ticks = counted
        .first()
        .and_then(|row| row["failed_ticks"].as_u64())
        .unwrap_or_default();
    if failed_ticks == 0 {
        return Ok((transaction, None));
    }
    let mut ends: Vec<SensorEvaluationFact> = Vec::new();
    let mut transaction = transaction;
    for order in ["ASC", "DESC"] {
        let rows;
        (transaction, rows) = query(
            transaction,
            &format!(
                "SELECT fact_json {streak} ORDER BY occurred_at {order}, event_id {order} LIMIT 1"
            ),
            vec![
                Parameter::Id(trigger_id),
                Parameter::Time(chrono::DateTime::UNIX_EPOCH),
            ],
        )
        .await?;
        let row = rows
            .first()
            .ok_or_else(|| ControlError::Invalid("failing streak changed while read".into()))?;
        ends.push(
            serde_json::from_value::<SensorEvaluationFact>(row["fact_json"].clone())
                .map_err(|error| ControlError::Invalid(error.to_string()))?,
        );
    }
    let [first, newest] = <[SensorEvaluationFact; 2]>::try_from(ends)
        .map_err(|_| ControlError::Invalid("failing streak has no ends".into()))?;
    let event = newest.request.event.as_ref();
    let reason = matches!(
        newest.outcome,
        SensorEvaluationOutcome::Abandoned {
            reason: AbandonReason::Unanswered
        }
    )
    .then(|| {
        format!(
            "the location didn't answer within {}s",
            newest.request.definition.timeout_seconds
        )
    });
    Ok((
        transaction,
        Some(SensorFailing {
            reason,
            since: first.request.requested_at,
            failed_ticks: usize::try_from(failed_ticks).unwrap_or(usize::MAX),
            event_id: event.map(|event| event.id().to_owned()),
            event_summary: event.map(SensorEvent::summary),
        }),
    ))
}
