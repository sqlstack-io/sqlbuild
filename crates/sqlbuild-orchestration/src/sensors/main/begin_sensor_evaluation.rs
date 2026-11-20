//! Commit the requested phase before evaluation, so crashes count as attempts; a last attempt an earlier release recorded in a form that no longer decodes counts as none, with a health problem until the location publishes again.

use chrono::TimeDelta;
use uuid::Uuid;

use crate::declarations::models::TriggerStatus;
use crate::platform::main::health::record_trigger_skipped::record_trigger_skipped;
use crate::scheduler::main::check_lease::check_lease;
use crate::scheduler::models::SchedulerLease;
use crate::sensors::_helpers::attempts::{defer_sensor, last_attempt, read_sensor};
use crate::sensors::_helpers::custom::request_evaluation_work;
use crate::sensors::_helpers::facts::{append_evaluation, definition_fingerprint};
use crate::sensors::constants::UNREADABLE_SENSOR_ATTEMPT;
use crate::sensors::main::read_sensor_cursor::read_sensor_cursor;
use crate::sensors::models::{SensorEvaluationOutcome, SensorEvaluationRequest};
use crate::store::errors::ControlError;
use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::models::{ControlStore, TransactionMode};

pub async fn begin_sensor_evaluation(
    store: &ControlStore,
    lease: &SchedulerLease,
    trigger_id: Uuid,
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
    let (transaction, cursor) = read_sensor_cursor(transaction, trigger_id).await?;
    let (mut transaction, previous) = last_attempt(transaction, trigger_id).await?;
    if let Some(Err(_)) = &previous {
        transaction = record_trigger_skipped(
            transaction,
            (trigger_id, sensor.snapshot_id),
            UNREADABLE_SENSOR_ATTEMPT,
        )
        .await?;
    }
    if let Some(Ok((previous, terminal))) = previous {
        if !terminal {
            if previous.request.epoch == lease.epoch
                && previous.request.snapshot_id == sensor.snapshot_id
                && previous.request.cursor == cursor
            {
                commit(transaction).await?;
                return Ok(Some(previous.request));
            }
            transaction = append_evaluation(
                transaction,
                &previous.request,
                SensorEvaluationOutcome::Abandoned,
            )
            .await?;
        }
        let next = previous.request.requested_at
            + TimeDelta::seconds(i64::from(
                sensor.definition_json.minimum_interval_seconds.get(),
            ));
        if next > now {
            commit(defer_sensor(transaction, trigger_id, next).await?).await?;
            return Ok(None);
        }
    }
    let definition_fingerprint = definition_fingerprint(&sensor.definition_json)?;
    let request = SensorEvaluationRequest {
        evaluation_id: Uuid::now_v7(),
        trigger_id: sensor.trigger_id,
        location_id: sensor.location_id,
        target: sensor.target,
        snapshot_id: sensor.snapshot_id,
        definition: sensor.definition_json,
        definition_fingerprint,
        cursor,
        epoch: lease.epoch,
        requested_at: now,
    };
    let transaction =
        append_evaluation(transaction, &request, SensorEvaluationOutcome::Requested).await?;
    let transaction = request_evaluation_work(transaction, &request).await?;
    commit(check_lease(transaction, lease).await?).await?;
    Ok(Some(request))
}
