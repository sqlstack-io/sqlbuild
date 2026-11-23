//! What a republish published, reported back to the server before its claim ends.

use std::future::Future;
use std::time::Duration;

use sqlbuild_orchestration::api::errors::ClientError;
use sqlbuild_orchestration::platform::models::{
    LocationWork, LocationWorkClaim, LocationWorkClient, LocationWorkOutcome, LocationWorkResult,
};
use sqlbuild_orchestration::store::errors::ControlError;
use tokio::sync::watch;
use tokio::time::Instant;
use uuid::Uuid;

use crate::argv::models::Stream;
use crate::native_commands::_helpers::command::terminal::write_output;
use crate::native_commands::_helpers::location::execution::request_republish;
use crate::native_commands::models::{
    DevControlState, LocationWatch, LocationWorkTurn, PendingRepublish,
};
use crate::native_commands::types::LoadOutcome;

/// How long one result is retried while the watcher waits.
const REPORT_RETRY_WINDOW: Duration = Duration::from_secs(30);

/// The outcome a compile gives the republish that asked for it.
fn republish_outcome(load: &LoadOutcome, generation: i64) -> LocationWorkOutcome {
    match load {
        Ok(published) => LocationWorkOutcome::Published {
            snapshot_id: published.snapshot_id,
            generation,
        },
        Err((_, error)) => LocationWorkOutcome::PublishFailed {
            generation,
            error: error.clone(),
        },
    }
}

/// The turn that hands a claimed republish to the watch loop, which publishes it; `None` for other work.
pub(crate) fn republish_turn(
    claim: &LocationWorkClaim,
    deadline: Instant,
) -> Option<LocationWorkTurn> {
    let LocationWork::Republish { requested_by } = &claim.request.work else {
        return None;
    };
    drop(write_output(&[(
        Stream::Stderr,
        &format!(
            "Republishing at the request of {requested_by}: {}\n",
            claim.request.reason
        ),
    )]));
    Some(LocationWorkTurn {
        files: Vec::new(),
        sync: false,
        claimed: true,
        republish: Some(PendingRepublish {
            request_id: claim.request.request_id,
            claim_id: claim.claim_id,
            deadline,
        }),
    })
}

/// `location` owing `claimed` a result, with the compile it waits for started; unchanged without one.
pub(crate) fn start_republish(
    republisher: &watch::Sender<DevControlState>,
    claimed: Option<PendingRepublish>,
    location: LocationWatch,
) -> LocationWatch {
    let Some(claimed) = claimed else {
        return location;
    };
    request_republish(republisher, true);
    LocationWatch {
        retry_at: Some(Instant::now()),
        republish: Some(claimed),
        ..location
    }
}

/// Report what a compile published to the republish that waits for it; `Some` when shutdown came first.
pub(crate) async fn settle_republish(
    session: (&LocationWorkClient, Uuid),
    owed: Option<PendingRepublish>,
    (load, generation): (&LoadOutcome, i64),
    stopped: impl Future<Output = Result<(), ControlError>>,
) -> Option<Result<(), ControlError>> {
    let pending: PendingRepublish = owed?;
    tokio::select! {
        result = stopped => Some(result),
        () = report_republish(session, pending, republish_outcome(load, generation)) => None,
    }
}

/// Send the result, retrying a lost send briefly so an unreachable server cannot stall the watcher.
async fn report_republish(
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
