//! The claimed run's execution contract, read before the worker compiles anything.

use crate::runs::models::RunExecution;
use crate::worker::errors::WorkerError;
use crate::worker::models::WorkerClient;

/// The execution the server holds for the worker's run and claim, such as its saved discovered sets.
pub async fn fetch_execution(client: &WorkerClient) -> Result<RunExecution, WorkerError> {
    client.execution().await
}
