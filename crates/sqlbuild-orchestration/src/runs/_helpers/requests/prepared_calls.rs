//! Plan, evaluate and list selectors over a prepared publication on a blocking thread, where its selector graphs are built and searched.

use std::sync::Arc;

use crate::runs::models::{CommandPlan, SelectionProblem, SelectionVocabulary};
use crate::runs::types::{PastRuns, PreparedPublication};
use crate::store::errors::ControlError;

/// [`PreparedPublication::plan`] off the async workers.
pub(in crate::runs) async fn plan_off_worker(
    prepared: &Arc<dyn PreparedPublication>,
    command: &[String],
    past: PastRuns,
) -> Result<Result<CommandPlan, String>, ControlError> {
    let (prepared, command) = (Arc::clone(prepared), command.to_vec());
    Ok(tokio::task::spawn_blocking(move || prepared.plan(&command, &past)).await?)
}

/// [`PreparedPublication::evaluate`] off the async workers.
pub(in crate::runs) async fn evaluate_off_worker(
    prepared: &Arc<dyn PreparedPublication>,
    (select, exclude): (&str, &str),
    past: PastRuns,
) -> Result<Result<Vec<String>, Box<SelectionProblem>>, ControlError> {
    let (prepared, select, exclude) = (Arc::clone(prepared), select.to_owned(), exclude.to_owned());
    Ok(tokio::task::spawn_blocking(move || prepared.evaluate((&select, &exclude), &past)).await?)
}

/// [`PreparedPublication::vocabulary`] off the async workers.
pub(in crate::runs) async fn vocabulary_off_worker(
    prepared: &Arc<dyn PreparedPublication>,
) -> Result<SelectionVocabulary, ControlError> {
    let prepared = Arc::clone(prepared);
    Ok(tokio::task::spawn_blocking(move || prepared.vocabulary()).await?)
}
