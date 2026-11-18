//! What a location's warehouse sync does when a read or a delivery fails, and how a page is delivered.

use std::future::Future;
use std::time::Duration;

use sqlbuild_orchestration::api::errors::ClientError;
use sqlbuild_orchestration::platform::models::{
    LocationWorkClient, LocationWorkPoll, WarehouseSyncFailure,
};
use sqlbuild_orchestration::store::errors::ControlError;
use tokio::time::Instant;

use crate::argv::models::Stream;
use crate::native_commands::_helpers::command::terminal::write_output;
use crate::native_commands::_helpers::location::execution::{
    acknowledge_metadata_page, close_metadata_reader, finish_metadata_page, location_warning,
};
use crate::native_commands::_helpers::location::sync_report::{
    metadata_health, read_failure_line, report_metadata_health,
};
use crate::native_commands::_helpers::location::work::refuse_rejected_sign_in;
use crate::native_commands::constants::{METADATA_FAILURE_LIMIT, METADATA_POLL_SECONDS};
use crate::native_commands::models::{LocationMetadataSync, ReadFailure};

/// A failed server call keeps progress and the exact pending page for a retry; a rejected sign-in stops the location.
pub(crate) fn retry_metadata(
    mut state: LocationMetadataSync,
    error: ClientError,
) -> Result<Option<LocationMetadataSync>, ControlError> {
    refuse_rejected_sign_in(&error)?;
    state.next_at = Instant::now() + Duration::from_secs(METADATA_POLL_SECONDS);
    state.reader = None;
    location_warning(
        "warehouse sync failed",
        format!("retrying in {METADATA_POLL_SECONDS}s: {error}"),
    );
    drop(write_output(&[(
        Stream::Stderr,
        &format!(
            "Warehouse history sync incomplete: {error}. Retaining progress and the exact pending page for retry.\n"
        ),
    )]));
    Ok(Some(state))
}

/// Report a failed read, then wait out a busy file, shrink the page, back off or stop the sync.
pub(crate) async fn failed_read(
    mut state: LocationMetadataSync,
    (client, poll): (&LocationWorkClient, &LocationWorkPoll),
    (started, failure): (Instant, ReadFailure),
    stopped: impl Future<Output = Result<(), ControlError>>,
) -> Result<Option<LocationMetadataSync>, ControlError> {
    let stage = if state.busy {
        WarehouseSyncFailure::Busy
    } else {
        WarehouseSyncFailure::Read
    };
    let halts: bool = !state.busy && state.halts(failure.page_size());
    tokio::select! {
        result = stopped => { result?; return Ok(None); },
        () = report_metadata_health(&state, (client, poll), (started, Some(stage), Some((&failure, halts)))) => {},
    }
    if state.busy {
        state.next_at = Instant::now() + Duration::from_secs(METADATA_POLL_SECONDS);
        return Ok(Some(state));
    }
    let line: String = read_failure_line(&state, &failure);
    let shrinks: bool = state.shrinks(failure.page_size());
    let outcome: String = match state.fail_read(failure.page_size()) {
        Some(wait) if shrinks => {
            format!(
                " Sync incomplete; retaining mirror and retrying a smaller page in {} s.\n",
                wait.as_secs()
            )
        }
        Some(wait) => {
            location_warning(
                "warehouse read failed",
                format!("retrying in {}s: {}", wait.as_secs(), failure.reason()),
            );
            format!(
                " Sync incomplete; retaining mirror and retrying in {} s.\n",
                wait.as_secs()
            )
        }
        None => {
            state = close_metadata_reader(state).await?;
            let resumes: &str = if state.defers_safety_sync() || state.safety_seconds == 0 {
                "after the next run, on request or after a publish"
            } else {
                "at the next safety sync, after the next run, on request or after a publish"
            };
            location_warning(
                "warehouse sync stopped",
                format!(
                    "{METADATA_FAILURE_LIMIT} failed reads in a row: {}; it resumes {resumes}",
                    failure.reason()
                ),
            );
            format!(
                " Synchronization stopped after {METADATA_FAILURE_LIMIT} failed reads in a row; it resumes {resumes}.\n"
            )
        }
    };
    drop(write_output(&[(
        Stream::Stderr,
        &format!("{line}{outcome}"),
    )]));
    Ok(Some(state))
}

/// Send the pending page and report its health; a failed delivery keeps the page for an exact retry.
pub(crate) async fn deliver_metadata_page(
    mut state: LocationMetadataSync,
    target: &str,
    (client, poll): (&LocationWorkClient, &LocationWorkPoll),
    (started, stopped): (Instant, impl Future<Output = Result<(), ControlError>>),
) -> Result<Option<LocationMetadataSync>, ControlError> {
    tokio::pin!(stopped);
    let Some(pending) = &state.pending else {
        return Ok(Some(state));
    };
    let response = tokio::select! {
        result = &mut stopped => { result?; return Ok(None); },
        response = client.sync_metadata(target, &pending.batch) => response,
    };
    let response = match response {
        Ok(response) => response,
        Err(error) => {
            tokio::select! {
            result = &mut stopped => { result?; return Ok(None); },
            () = metadata_health(&state, (client, poll), (started, Some(WarehouseSyncFailure::Delivery))) => {},
            }
            return retry_metadata(state, error);
        }
    };
    state.applied |= response.applied;
    tokio::select! {
        result = &mut stopped => { result?; return Ok(None); },
        () = metadata_health(&state, (client, poll), (started, None)) => {},
    }
    finish_metadata_page(acknowledge_metadata_page(state)).await
}
