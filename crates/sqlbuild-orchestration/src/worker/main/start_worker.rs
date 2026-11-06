//! Verify the baseline publication before any run overrides or warehouse mutation.

use uuid::Uuid;

use crate::runs::models::RunChange;
use crate::snapshots::main::verify_execution::verify_execution;
use crate::snapshots::models::SnapshotInput;
use crate::worker::_helpers::heartbeat::start_session;
use crate::worker::errors::WorkerError;
use crate::worker::models::{WorkerClient, WorkerSession};

/// Verify the baseline compile before planning, cloning, applying run settings or executing.
pub async fn start_worker(
    client: &WorkerClient,
    baseline: &SnapshotInput,
) -> Result<WorkerSession, WorkerError> {
    let expected = client.execution().await?;
    verify_execution(baseline, &expected)?;
    if client.heartbeat().await? {
        return Err(WorkerError::Cancelled);
    }
    let event_id = Uuid::new_v5(&expected.claim.claim_id, b"worker-started");
    let session = start_session(client.clone(), expected).await?;
    client.report(event_id, RunChange::Started).await?;
    Ok(session)
}
