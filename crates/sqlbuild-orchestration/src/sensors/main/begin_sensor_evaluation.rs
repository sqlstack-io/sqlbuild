//! Commit the requested phase before the location runs the function, so crashes count as attempts.

use chrono::TimeDelta;
use uuid::Uuid;

use crate::declarations::models::TriggerStatus;
use crate::scheduler::main::check_lease::check_lease;
use crate::scheduler::models::SchedulerLease;
use crate::sensors::_helpers::attempts::{defer_sensor, last_attempt, read_sensor};
use crate::sensors::_helpers::conditions::next_event;
use crate::sensors::_helpers::facts::{append_evaluation, commit_cursor, definition_fingerprint};
use crate::sensors::_helpers::work::request_evaluation_work;
use crate::sensors::main::read_sensor_cursor::read_sensor_cursor;
use crate::sensors::models::{
    EventScope, NextEvent, SensorEvaluationOutcome, SensorEvaluationRequest,
};
use crate::store::errors::ControlError;
use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::models::{ControlStore, TransactionMode};

/// The tick to run now, if the sensor is due and has an event or polls; `origin` starts the run links events carry.
pub async fn begin_sensor_evaluation(
    store: &ControlStore,
    lease: &SchedulerLease,
    (trigger_id, origin): (Uuid, Option<&str>),
) -> Result<Option<SensorEvaluationRequest>, ControlError> {
    let transaction = check_lease(begin(store, TransactionMode::Write).await?, lease).await?;
    let (transaction, sensor) = read_sensor(transaction, trigger_id).await?;
    let Some(sensor) = sensor else {
        return Ok(None);
    };
    if sensor.status != TriggerStatus::Running {
        return Ok(None);
    }
    let now = transaction.now;
    let interval = TimeDelta::seconds(i64::from(
        sensor.definition_json.minimum_interval_seconds.get(),
    ));
    let (transaction, cursor) = read_sensor_cursor(transaction, trigger_id).await?;
    let (mut transaction, previous) = last_attempt(transaction, trigger_id).await?;
    if let Some((previous, terminal)) = previous {
        let Some(phase) = terminal else {
            if previous.request.epoch == lease.epoch
                && previous.request.snapshot_id == sensor.snapshot_id
                && previous.request.cursor == cursor
            {
                commit(transaction).await?;
                return Ok(Some(previous.request));
            }
            let transaction = append_evaluation(
                transaction,
                &previous.request,
                SensorEvaluationOutcome::Abandoned,
            )
            .await?;
            commit(defer_sensor(transaction, trigger_id, now).await?).await?;
            return Ok(None);
        };
        let delivered =
            phase == SensorEvaluationOutcome::COMPLETED && previous.request.event.is_some();
        let next = previous.request.requested_at + interval;
        if !delivered && next > now {
            commit(defer_sensor(transaction, trigger_id, next).await?).await?;
            return Ok(None);
        }
    }
    let scope = EventScope {
        trigger_id,
        location_id: sensor.location_id,
        target: &sensor.target,
        snapshot_id: sensor.snapshot_id,
        origin,
    };
    let (event, cursor_after) = match &sensor.definition_json.when {
        None => (None, None),
        Some(condition) => {
            let found;
            (transaction, found) = next_event(transaction, scope, (condition, &cursor)).await?;
            match found {
                NextEvent::Found {
                    event,
                    cursor_after,
                } => (Some(event), Some(cursor_after)),
                NextEvent::Nothing {
                    cursor: moved,
                    more,
                } => {
                    if moved != cursor {
                        transaction = commit_cursor(transaction, trigger_id, &moved).await?;
                    }
                    let next = if more { now } else { now + interval };
                    let transaction = defer_sensor(transaction, trigger_id, next).await?;
                    commit(check_lease(transaction, lease).await?).await?;
                    return Ok(None);
                }
            }
        }
    };
    let definition_fingerprint = definition_fingerprint(&sensor.definition_json)?;
    let request = SensorEvaluationRequest {
        evaluation_id: Uuid::now_v7(),
        trigger_id: sensor.trigger_id,
        sensor: sensor.name,
        location_id: sensor.location_id,
        target: sensor.target,
        snapshot_id: sensor.snapshot_id,
        definition: sensor.definition_json,
        definition_fingerprint,
        cursor,
        event,
        cursor_after,
        epoch: lease.epoch,
        requested_at: now,
    };
    let transaction =
        append_evaluation(transaction, &request, SensorEvaluationOutcome::Requested).await?;
    let transaction = request_evaluation_work(transaction, &request).await?;
    commit(check_lease(transaction, lease).await?).await?;
    Ok(Some(request))
}
