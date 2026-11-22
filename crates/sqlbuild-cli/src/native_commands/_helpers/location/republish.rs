//! What a republish published, reported back to the server before its claim ends.

use std::time::Duration;

use sqlbuild_orchestration::api::errors::ClientError;
use sqlbuild_orchestration::platform::models::{
    LocationWorkClient, LocationWorkOutcome, LocationWorkResult,
};
use tokio::time::Instant;
use uuid::Uuid;

use crate::argv::models::Stream;
use crate::native_commands::_helpers::command::terminal::write_output;
use crate::native_commands::models::PendingRepublish;
use crate::native_commands::types::LoadOutcome;

/// How long one result is retried while the watcher waits.
const REPORT_RETRY_WINDOW: Duration = Duration::from_secs(30);

/// The outcome a compile gives the republish that asked for it.
pub(crate) fn republish_outcome(load: &LoadOutcome, generation: i64) -> LocationWorkOutcome {
    match load {
        Ok((_, _, snapshot_id)) => LocationWorkOutcome::Published {
            snapshot_id: *snapshot_id,
            generation,
        },
        Err((_, error)) => LocationWorkOutcome::PublishFailed {
            generation,
            error: error.clone(),
        },
    }
}

/// Send the result, retrying a lost send briefly so an unreachable server cannot stall the watcher.
pub(crate) async fn report_republish(
    (client, session_id): (&LocationWorkClient, Uuid),
    pending: PendingRepublish,
    outcome: LocationWorkOutcome,
) {
    let result = LocationWorkResult {
        claim_id: pending.claim_id,
        session_id,
        outcome,
    };
    let retry_until: Instant = pending.deadline.min(Instant::now() + REPORT_RETRY_WINDOW);
    while Instant::now() < retry_until {
        let sent =
            tokio::time::timeout_at(retry_until, client.complete(pending.request_id, &result));
        let Ok(sent) = sent.await else {
            break;
        };
        match sent {
            Ok(_) => {
                drop(write_output(&[(
                    Stream::Stderr,
                    "Republish result reported.\n",
                )]));
                return;
            }
            Err(error) => {
                drop(write_output(&[(
                    Stream::Stderr,
                    &format!("Reporting the republish result failed: {error}\n"),
                )]));
                if matches!(error, ClientError::Rejected(status) if status.is_client_error()) {
                    return;
                }
            }
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    drop(write_output(&[(
        Stream::Stderr,
        "The republish's result could not be reported to the server.\n",
    )]));
}
