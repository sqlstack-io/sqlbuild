//! Commit the requested phase before evaluation, so crashes count as attempts.

use chrono::TimeDelta;
use sqlbuild_recall::records::main::sha256_hex::sha256_hex;
use uuid::Uuid;

use crate::declarations::models::TriggerStatus;
use crate::scheduler::main::check_lease::check_lease;
use crate::scheduler::models::SchedulerLease;
use crate::sensors::_helpers::attempts::{defer_sensor, last_attempt, read_sensor};
use crate::sensors::_helpers::facts::append_evaluation;
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
    if let Some((previous, terminal)) = previous {
        if !terminal {
            if previous.request.epoch == lease.epoch
                && previous.request.snapshot_id == sensor.snapshot_id
                && previous.request.cursor == cursor
            {
                commit(transaction).await?;
                return Ok(Some(previous.request));
            }
            (transaction, _) = append_evaluation(
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
    let definition_fingerprint = sha256_hex(
        &serde_json::to_vec(&sensor.definition_json)
            .map_err(|error| ControlError::Invalid(error.to_string()))?,
    );
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
    let (transaction, _) =
        append_evaluation(transaction, &request, SensorEvaluationOutcome::Requested).await?;
    commit(check_lease(transaction, lease).await?).await?;
    Ok(Some(request))
}
