//! Verify the baseline publication before any run overrides or warehouse mutation.

use uuid::Uuid;

use crate::runs::models::{RunChange, RunOutcome};
use crate::snapshots::main::publication::verify_execution::verify_execution;
use crate::snapshots::models::SnapshotInput;
use crate::worker::_helpers::heartbeat::start_session;
use crate::worker::constants::{REFUSAL_ATTEMPTS, REFUSAL_RETRY_DELAY, STORE_BUSY_STATUS};
use crate::worker::errors::WorkerError;
use crate::worker::models::{WorkerClient, WorkerSession};

/// Verify the baseline compile before planning, cloning, applying run settings or executing.
pub async fn start_worker(
    client: &WorkerClient,
    baseline: &SnapshotInput,
) -> Result<WorkerSession, WorkerError> {
    let expected = match client.execution().await {
        Err(WorkerError::UnreadableExecution(detail)) => {
            let error = WorkerError::NewerServer {
                version: baseline.sqb_version.clone(),
                detail,
            };
            refuse_unreadable(client, &error).await;
            return Err(error);
        }
        expected => expected?,
    };
    verify_execution(baseline, &expected)?;
    if client.heartbeat().await? {
        return Err(WorkerError::Cancelled);
    }
    let event_id = Uuid::new_v5(&expected.claim.claim_id, b"worker-started");
    let session = start_session(client.clone(), expected).await?;
    client.report(event_id, RunChange::Started).await?;
    Ok(session)
}

/// Fail the run with why its request can't be read, retrying a busy store or a lost response with the same event; the supervisor's own exit reason only applies if this report doesn't land.
async fn refuse_unreadable(client: &WorkerClient, error: &WorkerError) {
    let event_id = Uuid::new_v5(&client.claim_id, b"worker-unreadable-request");
    let change = RunChange::Finished {
        outcome: RunOutcome::Failed,
        reason: error.to_string(),
        exit_code: None,
    };
    for attempt in 1..=REFUSAL_ATTEMPTS {
        match client.report(event_id, change.clone()).await {
            Err(WorkerError::Transport(_)) if attempt < REFUSAL_ATTEMPTS => {}
            Err(WorkerError::Rejected(status))
                if attempt < REFUSAL_ATTEMPTS && status.as_u16() == STORE_BUSY_STATUS => {}
            _ => return,
        }
        tokio::time::sleep(REFUSAL_RETRY_DELAY).await;
    }
}
