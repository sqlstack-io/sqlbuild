//! A sensor is failing from the first failed, timed-out or unanswered tick after its last good one; other abandoned ticks don't count either way.

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::sensors::constants::{FAILING_TICK_PHASES, UNREADABLE_TICK};
use crate::sensors::models::{
    AbandonReason, SensorEvaluationFact, SensorEvaluationOutcome, SensorEvent, SensorFailing,
};
use crate::sensors::types::StreakEnd;
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};
use crate::store::types::Row;

/// Since when the sensor's ticks have failed, how many have, and the event and reason of the newest; three indexed reads however long the streak. An end that no longer decodes gives its recorded time and event, and says so.
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
    let mut ends: Vec<StreakEnd> = Vec::new();
    let mut transaction = transaction;
    for order in ["ASC", "DESC"] {
        let rows;
        (transaction, rows) = query(
            transaction,
            &format!(
                "SELECT fact_json, occurred_at, subject_event_id {streak} \
                 ORDER BY occurred_at {order}, event_id {order} LIMIT 1"
            ),
            vec![
                Parameter::Id(trigger_id),
                Parameter::Time(DateTime::UNIX_EPOCH),
            ],
        )
        .await?;
        let row = rows
            .first()
            .ok_or_else(|| ControlError::Invalid("failing streak changed while read".into()))?;
        ends.push(streak_end(row)?);
    }
    let [first, newest] = <[StreakEnd; 2]>::try_from(ends)
        .map_err(|_| ControlError::Invalid("failing streak has no ends".into()))?;
    let (since, _) = first;
    let (_, (reason, event_id, event_summary)) = newest;
    Ok((
        transaction,
        Some(SensorFailing {
            reason,
            since,
            failed_ticks: usize::try_from(failed_ticks).unwrap_or(usize::MAX),
            event_id,
            event_summary,
        }),
    ))
}

/// When the tick started, then why it failed, its event and that event's summary; from the columns when its fact no longer decodes.
fn streak_end(row: &Row) -> Result<StreakEnd, ControlError> {
    let Ok(fact) = serde_json::from_value::<SensorEvaluationFact>(row["fact_json"].clone()) else {
        let recorded: DateTime<Utc> = serde_json::from_value(row["occurred_at"].clone())
            .map_err(|error| ControlError::Invalid(error.to_string()))?;
        let event_id: Option<String> = row["subject_event_id"].as_str().map(str::to_owned);
        return Ok((recorded, (Some(UNREADABLE_TICK.into()), event_id, None)));
    };
    let reason: Option<String> = matches!(
        fact.outcome,
        SensorEvaluationOutcome::Abandoned {
            reason: AbandonReason::Unanswered
        }
    )
    .then(|| {
        format!(
            "the location didn't answer within {}s",
            fact.request.definition.timeout_seconds
        )
    });
    let event: Option<&SensorEvent> = fact.request.event.as_ref();
    Ok((
        fact.request.requested_at,
        (
            reason,
            event.map(|event| event.id().to_owned()),
            event.map(SensorEvent::summary),
        ),
    ))
}
