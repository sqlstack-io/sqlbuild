//! Cursor, outcome, deduplicated requests and queued runs commit together.

use chrono::TimeDelta;

use crate::declarations::models::TriggerStatus;
use crate::scheduler::main::check_lease::check_lease;
use crate::scheduler::models::SchedulerLease;
use crate::sensors::_helpers::attempts::{defer_sensor, last_attempt, read_sensor};
use crate::sensors::_helpers::facts::{append_evaluation, commit_cursor};
use crate::sensors::_helpers::platform::consume_platform_events;
use crate::sensors::_helpers::runs::consume_run_events;
use crate::sensors::main::read_sensor_cursor::read_sensor_cursor;
use crate::sensors::models::{SensorCondition, SensorEvaluationOutcome, SensorEvaluationRequest};
use crate::store::errors::ControlError;
use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::models::{ControlStore, TransactionMode};

pub async fn complete_builtin_sensor(
    store: &ControlStore,
    lease: &SchedulerLease,
    evaluation: &SensorEvaluationRequest,
) -> Result<usize, ControlError> {
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
        return Ok(0);
    }
    let (transaction, sensor) = read_sensor(transaction, evaluation.trigger_id).await?;
    let (transaction, cursor) = read_sensor_cursor(transaction, evaluation.trigger_id).await?;
    if sensor.is_none_or(|sensor| {
        sensor.status != TriggerStatus::Running || sensor.snapshot_id != evaluation.snapshot_id
    }) || cursor != evaluation.cursor
    {
        let (transaction, _) =
            append_evaluation(transaction, evaluation, SensorEvaluationOutcome::Abandoned).await?;
        commit(transaction).await?;
        return Ok(0);
    }
    let (transaction, (cursor, run_ids)) = match &evaluation.definition.when {
        SensorCondition::PlatformEvent { kinds } => {
            consume_platform_events(transaction, evaluation, kinds).await?
        }
        SensorCondition::RunEvent { job_key, kinds } => {
            consume_run_events(transaction, evaluation, job_key, kinds).await?
        }
    };
    let count = run_ids.len();
    let (transaction, (event_id, storage_id)) = append_evaluation(
        transaction,
        evaluation,
        SensorEvaluationOutcome::Completed {
            cursor: cursor.clone(),
            run_ids,
        },
    )
    .await?;
    let transaction = commit_cursor(
        transaction,
        (evaluation.trigger_id, event_id, storage_id),
        &cursor,
    )
    .await?;
    let next = evaluation.requested_at
        + TimeDelta::seconds(i64::from(
            evaluation.definition.minimum_interval_seconds.get(),
        ));
    let transaction = defer_sensor(transaction, evaluation.trigger_id, next).await?;
    commit(check_lease(transaction, lease).await?).await?;
    Ok(count)
}
