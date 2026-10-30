//! Verify the baseline publication before any run overrides or warehouse mutation.

use uuid::Uuid;

use crate::runs::models::{RunChange, RunExecution};
use crate::snapshots::main::verify_execution::verify_execution;
use crate::snapshots::models::SnapshotInput;
use crate::worker::errors::WorkerError;
use crate::worker::models::WorkerClient;

/// Verify the baseline compile before planning, cloning, applying run settings or executing.
pub async fn start_worker(
    client: &WorkerClient,
    baseline: &SnapshotInput,
) -> Result<RunExecution, WorkerError> {
    let expected = client.execution().await?;
    verify_execution(baseline, &expected)?;
    if client.heartbeat().await? {
        return Err(WorkerError::Cancelled);
    }
    client
        .report(
            Uuid::new_v5(&expected.claim.claim_id, b"worker-started"),
            RunChange::Started,
        )
        .await?;
    Ok(expected)
}
