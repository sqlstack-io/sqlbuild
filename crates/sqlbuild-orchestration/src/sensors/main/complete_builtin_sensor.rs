//! Cursor, outcome, deduplicated requests and queued runs commit together.

use crate::scheduler::models::SchedulerLease;
use crate::sensors::_helpers::completion::{finish_evaluation, pending_evaluation};
use crate::sensors::_helpers::conditions::consume_condition;
use crate::sensors::models::{SensorEvaluationOutcome, SensorEvaluationRequest};
use crate::store::errors::ControlError;
use crate::store::models::ControlStore;

pub async fn complete_builtin_sensor(
    store: &ControlStore,
    lease: &SchedulerLease,
    evaluation: &SensorEvaluationRequest,
) -> Result<usize, ControlError> {
    let Some(transaction) = pending_evaluation(store, lease, evaluation).await? else {
        return Ok(0);
    };
    let (transaction, (cursor, requests)) = consume_condition(transaction, evaluation).await?;
    let count = requests.run_ids.len();
    finish_evaluation(
        transaction,
        lease,
        evaluation,
        SensorEvaluationOutcome::Completed {
            cursor,
            run_ids: requests.run_ids,
            skipped: requests.skipped,
        },
    )
    .await?;
    Ok(count)
}
