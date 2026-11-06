//! A location result cannot queue runs or advance a cursor until a fenced scheduler commits it.

use chrono::TimeDelta;

use crate::scheduler::main::check_lease::check_lease;
use crate::scheduler::models::SchedulerLease;
use crate::sensors::_helpers::attempts::defer_sensor;
use crate::sensors::_helpers::completion::{finish_evaluation, pending_evaluation};
use crate::sensors::_helpers::custom::read_evaluation_result;
use crate::sensors::constants::CUSTOM_SENSOR_RECHECK_SECONDS;
use crate::sensors::models::{SensorCondition, SensorEvaluationOutcome, SensorEvaluationRequest};
use crate::store::errors::ControlError;
use crate::store::main::commit::commit;
use crate::store::models::ControlStore;

pub async fn complete_custom_sensor(
    store: &ControlStore,
    lease: &SchedulerLease,
    evaluation: &SensorEvaluationRequest,
) -> Result<usize, ControlError> {
    if !matches!(evaluation.definition.when, SensorCondition::Custom { .. }) {
        return Err(ControlError::Invalid(
            "custom sensor evaluation required".into(),
        ));
    }
    let Some(transaction) = pending_evaluation(store, lease, evaluation).await? else {
        return Ok(0);
    };
    let (transaction, outcome) = read_evaluation_result(transaction, evaluation).await?;
    let Some(outcome) = outcome else {
        let next = transaction.now + TimeDelta::seconds(CUSTOM_SENSOR_RECHECK_SECONDS);
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
