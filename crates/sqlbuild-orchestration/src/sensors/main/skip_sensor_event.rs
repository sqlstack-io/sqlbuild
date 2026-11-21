//! An operator marks the event a sensor is stuck on delivered; a completed tick in its history names who skipped it and why.

use uuid::Uuid;

use crate::declarations::models::TriggerChangeRequest;
use crate::sensors::_helpers::ticks::attempts::last_attempt;
use crate::sensors::_helpers::ticks::facts::{append_evaluation, commit_cursor, record_delivery};
use crate::sensors::main::read_sensor_cursor::read_sensor_cursor;
use crate::sensors::models::{
    AbandonReason, SensorCursor, SensorEvaluationOutcome, SensorEvaluationRequest,
};
use crate::store::errors::ControlError;
use crate::store::models::ControlTransaction;

/// Skip `event`, which the sensor's newest tick, finished or still out, hasn't handled and its cursor still waits on: the cursor after it; `paused` says no next tick comes.
pub(crate) async fn skip_sensor_event(
    transaction: ControlTransaction,
    request: &TriggerChangeRequest,
    (event, paused): (&str, bool),
) -> Result<(ControlTransaction, SensorCursor), ControlError> {
    let (transaction, last) = last_attempt(transaction, request.trigger_id).await?;
    let (stuck, terminal) = match last {
        None => {
            return Err(ControlError::Invalid(
                "the sensor hasn't been called with an event yet".into(),
            ));
        }
        Some(Err(_)) if paused => {
            return Err(ControlError::Invalid(
                "the sensor's newest tick can't be read, and the sensor is paused, so no next tick \
                 checks it again; resume the sensor, or reset its cursor"
                    .into(),
            ));
        }
        Some(Err(_)) => {
            return Err(ControlError::Invalid(
                "the sensor's newest tick can't be read; its next tick checks it again from scratch"
                    .into(),
            ));
        }
        Some(Ok(last)) => last,
    };
    let stuck_on = stuck
        .request
        .event
        .as_ref()
        .map(|stuck| stuck.id().to_owned());
    if stuck_on.as_deref() != Some(event) {
        return Err(ControlError::Invalid(match stuck_on {
            Some(stuck_on) => format!("the sensor is waiting on {stuck_on}, not {event}"),
            None => "a polling sensor has no event to skip".into(),
        }));
    }
    if terminal.as_deref() == Some(SensorEvaluationOutcome::COMPLETED) {
        return Err(ControlError::Invalid(format!(
            "the sensor already handled {event}"
        )));
    }
    let (transaction, cursor) = read_sensor_cursor(transaction, request.trigger_id).await?;
    if cursor != stuck.request.cursor {
        return Err(ControlError::Invalid(format!(
            "the sensor has moved past {event} since its last tick"
        )));
    }
    let transaction = if terminal.is_none() {
        append_evaluation(
            transaction,
            &stuck.request,
            SensorEvaluationOutcome::Abandoned {
                reason: AbandonReason::Skipped,
            },
        )
        .await?
    } else {
        transaction
    };
    let skipped = SensorEvaluationRequest {
        evaluation_id: Uuid::now_v7(),
        requested_at: transaction.now,
        ..stuck.request
    };
    let after = skipped
        .cursor_after
        .clone()
        .unwrap_or_else(|| skipped.cursor.clone());
    let transaction =
        append_evaluation(transaction, &skipped, SensorEvaluationOutcome::Requested).await?;
    let transaction = append_evaluation(
        transaction,
        &skipped,
        SensorEvaluationOutcome::Completed {
            cursor: after.clone(),
            run_ids: Vec::new(),
            skipped: Vec::new(),
            skip_reason: Some(request.reason.clone()),
            skipped_by: Some(request.actor.clone()),
        },
    )
    .await?;
    let transaction = commit_cursor(transaction, request.trigger_id, &after).await?;
    let transaction = record_delivery(transaction, &skipped).await?;
    Ok((transaction, after))
}
