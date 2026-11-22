//! Commit the requested phase before the location runs the function, so crashes count as attempts; a last attempt an earlier release recorded in a form that no longer decodes counts as none, with a health problem until a check completes.

use chrono::TimeDelta;
use uuid::Uuid;

use crate::declarations::models::TriggerStatus;
use crate::platform::main::health::record_trigger_problem::record_trigger_problem;
use crate::platform::models::PlatformEvent;
use crate::scheduler::main::check_lease::check_lease;
use crate::scheduler::models::SchedulerLease;
use crate::sensors::_helpers::ticks::attempts::{defer_sensor, read_sensor};
use crate::sensors::_helpers::ticks::facts::{append_evaluation, definition_fingerprint};
use crate::sensors::_helpers::ticks::schedule::{previous_tick, tick_event};
use crate::sensors::_helpers::ticks::work::request_evaluation_work;
use crate::sensors::main::read_sensor_cursor::read_sensor_cursor;
use crate::sensors::models::{
    EventScope, PreviousTick, SensorEvaluationOutcome, SensorEvaluationRequest,
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
    let interval = TimeDelta::seconds(i64::from(
        sensor.definition_json.minimum_interval_seconds.get(),
    ));
    let (transaction, cursor) = read_sensor_cursor(transaction, trigger_id).await?;
    let (transaction, previous) =
        previous_tick(transaction, lease, (&sensor, &cursor), interval).await?;
    let rechecked: bool = matches!(previous, PreviousTick::Recheck);
    match previous {
        PreviousTick::Resume(request) => {
            commit(transaction).await?;
            return Ok(Some(*request));
        }
        PreviousTick::Wait(next) => {
            commit(defer_sensor(transaction, trigger_id, next).await?).await?;
            return Ok(None);
        }
        PreviousTick::Proceed | PreviousTick::Recheck => {}
    }
    let scope = EventScope {
        trigger_id,
        location_id: sensor.location_id,
        target: &sensor.target,
        snapshot_id: sensor.snapshot_id,
        origin,
    };
    let (transaction, found) = tick_event(transaction, scope, (&sensor, &cursor), interval).await?;
    let Some((event, cursor_after)) = found else {
        commit(check_lease(transaction, lease).await?).await?;
        return Ok(None);
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
        requested_at: transaction.now,
    };
    let mut transaction =
        append_evaluation(transaction, &request, SensorEvaluationOutcome::Requested).await?;
    if rechecked {
        (transaction, _) = record_trigger_problem(
            transaction,
            (
                PlatformEvent::SensorRechecked {
                    trigger_id: request.trigger_id,
                    snapshot_id: request.snapshot_id,
                    evaluation_id: request.evaluation_id,
                },
                None,
            ),
        )
        .await?;
    }
    let transaction = request_evaluation_work(transaction, &request).await?;
    commit(check_lease(transaction, lease).await?).await?;
    Ok(Some(request))
}
