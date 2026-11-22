//! When a due sensor's next tick may start: resume the one in flight, wait out a failure, or find its event; a newest tick that no longer decodes counts as none.

use chrono::{DateTime, TimeDelta, Utc};

use crate::scheduler::models::SchedulerLease;
use crate::sensors::_helpers::events::conditions::next_event;
use crate::sensors::_helpers::ticks::attempts::{defer_sensor, last_attempt};
use crate::sensors::_helpers::ticks::facts::{append_evaluation, commit_cursor};
use crate::sensors::models::{
    AbandonReason, DueSensor, EventScope, NextEvent, PreviousTick, SensorCursor,
    SensorEvaluationOutcome, SensorEvent,
};
use crate::store::errors::ControlError;
use crate::store::models::ControlTransaction;

/// What the newest tick leaves for this one; an in-flight tick that no longer fits is recorded as abandoned.
pub(crate) async fn previous_tick(
    transaction: ControlTransaction,
    lease: &SchedulerLease,
    (sensor, cursor): (&DueSensor, &SensorCursor),
    interval: TimeDelta,
) -> Result<(ControlTransaction, PreviousTick), ControlError> {
    let (mut transaction, previous) = last_attempt(transaction, sensor.trigger_id).await?;
    let (previous, terminal) = match previous {
        None => return Ok((transaction, PreviousTick::Proceed)),
        Some(Err(_)) => return Ok((transaction, PreviousTick::Recheck)),
        Some(Ok(previous)) => previous,
    };
    if terminal.is_none() {
        if previous.request.epoch == lease.epoch
            && previous.request.snapshot_id == sensor.snapshot_id
            && &previous.request.cursor == cursor
        {
            return Ok((
                transaction,
                PreviousTick::Resume(Box::new(previous.request)),
            ));
        }
        let reason = if previous.request.epoch != lease.epoch {
            AbandonReason::LeaderChanged
        } else if previous.request.snapshot_id != sensor.snapshot_id {
            AbandonReason::Republished
        } else {
            AbandonReason::CursorReset
        };
        transaction = append_evaluation(
            transaction,
            &previous.request,
            SensorEvaluationOutcome::Abandoned { reason },
        )
        .await?;
    }
    let retry_now = match terminal.as_deref() {
        None => previous.request.epoch == lease.epoch,
        Some(SensorEvaluationOutcome::ABANDONED) => true,
        Some(SensorEvaluationOutcome::COMPLETED) => previous.request.event.is_some(),
        Some(_) => false,
    };
    let next = previous.request.requested_at + interval;
    if !retry_now && next > transaction.now {
        return Ok((transaction, PreviousTick::Wait(next)));
    }
    Ok((transaction, PreviousTick::Proceed))
}

/// The event the tick is for and the cursor past it, `(None, None)` for a polling sensor; `None` defers a sensor with nothing to deliver.
pub(crate) async fn tick_event(
    mut transaction: ControlTransaction,
    scope: EventScope<'_>,
    (sensor, cursor): (&DueSensor, &SensorCursor),
    interval: TimeDelta,
) -> Result<
    (
        ControlTransaction,
        Option<(Option<SensorEvent>, Option<SensorCursor>)>,
    ),
    ControlError,
> {
    let Some(condition) = &sensor.definition_json.when else {
        return Ok((transaction, Some((None, None))));
    };
    let found;
    (transaction, found) = next_event(transaction, scope, (condition, cursor)).await?;
    match found {
        NextEvent::Found {
            event,
            cursor_after,
        } => Ok((transaction, Some((Some(event), Some(cursor_after))))),
        NextEvent::Nothing {
            cursor: moved,
            more,
        } => {
            if &moved != cursor {
                transaction = commit_cursor(transaction, sensor.trigger_id, &moved).await?;
            }
            let next: DateTime<Utc> = if more {
                transaction.now
            } else {
                transaction.now + interval
            };
            Ok((
                defer_sensor(transaction, sensor.trigger_id, next).await?,
                None,
            ))
        }
    }
}
