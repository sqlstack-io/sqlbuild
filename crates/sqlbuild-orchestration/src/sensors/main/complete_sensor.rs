//! A location's result cannot queue runs or advance a cursor until the fenced scheduler commits it.

use chrono::TimeDelta;

use crate::scheduler::main::check_lease::check_lease;
use crate::scheduler::models::SchedulerLease;
use crate::sensors::_helpers::attempts::defer_sensor;
use crate::sensors::_helpers::completion::{finish_evaluation, pending_evaluation};
use crate::sensors::_helpers::work::read_evaluation_result;
use crate::sensors::constants::SENSOR_RESULT_RECHECK_SECONDS;
use crate::sensors::models::{SensorEvaluationOutcome, SensorEvaluationRequest};
use crate::store::errors::ControlError;
use crate::store::main::commit::commit;
use crate::store::models::ControlStore;

/// Apply the location's answer to the tick, if it has one yet; the runs it started.
pub async fn complete_sensor(
    store: &ControlStore,
    lease: &SchedulerLease,
    evaluation: &SensorEvaluationRequest,
) -> Result<usize, ControlError> {
    let Some(transaction) = pending_evaluation(store, lease, evaluation).await? else {
        return Ok(0);
    };
    let (transaction, outcome) = read_evaluation_result(transaction, evaluation).await?;
    let Some(outcome) = outcome else {
        let next = transaction.now + TimeDelta::seconds(SENSOR_RESULT_RECHECK_SECONDS);
        let transaction = defer_sensor(transaction, evaluation.trigger_id, next).await?;
        commit(check_lease(transaction, lease).await?).await?;
        return Ok(0);
    };
    let count = match &outcome {
        SensorEvaluationOutcome::Completed { run_ids, .. } => run_ids.len(),
        _ => 0,
    };
    finish_evaluation(transaction, lease, evaluation, outcome).await?;
    Ok(count)
}
