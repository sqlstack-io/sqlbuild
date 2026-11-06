//! Every evaluation result uses the same lease, snapshot, cursor and terminal boundary.

use chrono::TimeDelta;

use crate::declarations::models::TriggerStatus;
use crate::scheduler::main::check_lease::check_lease;
use crate::scheduler::models::SchedulerLease;
use crate::sensors::_helpers::attempts::{defer_sensor, last_attempt, read_sensor};
use crate::sensors::_helpers::facts::{append_evaluation, commit_cursor};
use crate::sensors::main::read_sensor_cursor::read_sensor_cursor;
use crate::sensors::models::{SensorEvaluationOutcome, SensorEvaluationRequest};
use crate::store::errors::ControlError;
use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::models::{ControlStore, ControlTransaction, TransactionMode};

pub(crate) async fn pending_evaluation(
    store: &ControlStore,
    lease: &SchedulerLease,
    evaluation: &SensorEvaluationRequest,
) -> Result<Option<ControlTransaction>, ControlError> {
    let transaction = check_lease(begin(store, TransactionMode::Write).await?, lease).await?;
    if evaluation.epoch != lease.epoch {
        return Err(ControlError::Fenced);
    }
    let (transaction, previous) = last_attempt(transaction, evaluation.trigger_id).await?;
    let Some((previous, terminal)) = previous else {
        return Err(ControlError::Fenced);
    };
    if previous.request != *evaluation {
        return Err(ControlError::Fenced);
    }
    if terminal {
        commit(transaction).await?;
        return Ok(None);
    }
    let (transaction, sensor) = read_sensor(transaction, evaluation.trigger_id).await?;
    let (transaction, cursor) = read_sensor_cursor(transaction, evaluation.trigger_id).await?;
    if sensor.is_none_or(|sensor| {
        sensor.status != TriggerStatus::Running || sensor.snapshot_id != evaluation.snapshot_id
    }) || cursor != evaluation.cursor
    {
        let transaction =
            append_evaluation(transaction, evaluation, SensorEvaluationOutcome::Abandoned).await?;
        commit(transaction).await?;
        return Ok(None);
    }
    Ok(Some(transaction))
}

pub(crate) async fn finish_evaluation(
    transaction: ControlTransaction,
    lease: &SchedulerLease,
    evaluation: &SensorEvaluationRequest,
    outcome: SensorEvaluationOutcome,
) -> Result<(), ControlError> {
    let cursor = match &outcome {
        SensorEvaluationOutcome::Completed { cursor, .. } => Some(cursor.clone()),
        _ => None,
    };
    let mut transaction = append_evaluation(transaction, evaluation, outcome).await?;
    if let Some(cursor) = cursor {
        transaction = commit_cursor(transaction, evaluation.trigger_id, &cursor).await?;
    }
    let next = evaluation.requested_at
        + TimeDelta::seconds(i64::from(
            evaluation.definition.minimum_interval_seconds.get(),
        ));
    let transaction = defer_sensor(transaction, evaluation.trigger_id, next).await?;
    commit(check_lease(transaction, lease).await?).await
}
