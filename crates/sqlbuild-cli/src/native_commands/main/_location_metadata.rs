//! Isolated read-only pages and exact acknowledgement retries share the published input fence.

use std::future::Future;
use std::path::Path;
use std::time::Duration;

use sqlbuild_orchestration::api::errors::ClientError;
use sqlbuild_orchestration::platform::models::{LocationWorkClient, LocationWorkPoll};
use sqlbuild_orchestration::store::errors::ControlError;
use sqlbuild_orchestration::warehouse_metadata::models::{
    WarehouseHistoryKind, WarehouseHistoryPage, WarehouseWatermarkRead,
};
use tokio::time::Instant;

use crate::argv::models::Stream;
use crate::native_commands::_helpers::execution::{
    acknowledge_metadata_page, deliver_metadata_page, freeze_history_page, history_request,
    location_result,
};
use crate::native_commands::_helpers::paths::location_inputs;
use crate::native_commands::_helpers::terminal::write_output;
use crate::native_commands::constants::{
    METADATA_OVERLAP_SECONDS, METADATA_POLL_SECONDS, METADATA_REPLY_BYTES,
};
use crate::native_commands::models::{LocationMetadataSync, LocationServe, LocationWatch};

/// One bounded source or mirror page leaves named work polling a turn between pages.
pub(super) async fn sync_location_metadata(
    mut state: LocationMetadataSync,
    (options, script, watch): (&LocationServe, &Path, &LocationWatch),
    (client, poll): (&LocationWorkClient, &LocationWorkPoll),
    stopped: impl Future<Output = Result<(), ControlError>>,
) -> Result<Option<LocationMetadataSync>, ControlError> {
    if Instant::now() < state.next_at
        || state.locations.is_empty()
        || location_inputs(&options.project_dir, &watch.rules, &watch.local_files) != watch.observed
    {
        return Ok(Some(state));
    }
    tokio::pin!(stopped);
    if state.pending.is_none() {
        if !state.watermark_loaded {
            if state.index == 0 {
                state.reconcile = Instant::now() >= state.reconcile_at;
            }
            if !state.reconcile {
                let request = WarehouseWatermarkRead {
                    session_id: poll.session_id,
                    generation: state.generation,
                    location: state.locations[state.index / 2].clone(),
                    kind: if state.index.is_multiple_of(2) {
                        WarehouseHistoryKind::Fingerprints
                    } else {
                        WarehouseHistoryKind::SourceFreshness
                    },
                };
                let watermark = tokio::select! {
                    result = &mut stopped => { result?; return Ok(None); },
                    result = client.metadata_watermark(&options.target, &request) => result,
                };
                let watermark = match watermark {
                    Ok(watermark) => watermark,
                    Err(error) => return retry_metadata(state, error),
                };
                state.observed_since = watermark.and_then(|time| {
                    time.checked_sub_signed(chrono::Duration::seconds(METADATA_OVERLAP_SECONDS))
                });
            }
            state.watermark_loaded = true;
        }
        let Some(page) = read_metadata_page(
            (options, script),
            history_request(&state)?,
            stopped.as_mut(),
        )
        .await?
        else {
            return Ok(None);
        };
        let Some(page) = page else {
            state.limit = (state.limit / 2).max(1);
            state.next_at = Instant::now() + Duration::from_secs(METADATA_POLL_SECONDS);
            drop(write_output(&[(
                Stream::Stderr,
                "Warehouse history read failed; retaining mirror and retrying a bounded page.\n",
            )]));
            return Ok(Some(state));
        };
        if location_inputs(&options.project_dir, &watch.rules, &watch.local_files) != watch.observed
        {
            return Ok(Some(state));
        }
        state = freeze_history_page(state, page, poll)?;
    }
    let Some(pending) = &state.pending else {
        return Ok(Some(state));
    };
    let response = tokio::select! {
        result = &mut stopped => { result?; return Ok(None); },
        response = deliver_metadata_page((client, &options.target), &pending.body, state.index.is_multiple_of(2)) => response,
    };
    let response = match response {
        Ok(response) => response,
        Err(error) => return retry_metadata(state, error),
    };
    acknowledge_metadata_page(state, response).map(Some)
}

fn retry_metadata(
    mut state: LocationMetadataSync,
    error: ClientError,
) -> Result<Option<LocationMetadataSync>, ControlError> {
    if matches!(error, ClientError::Configuration(_))
        || matches!(error, ClientError::Rejected(status) if matches!(status.as_u16(), 401 | 403))
    {
        return Err(ControlError::Invalid(error.to_string()));
    }
    state.next_at = Instant::now() + Duration::from_secs(METADATA_POLL_SECONDS);
    drop(write_output(&[(
        Stream::Stderr,
        &format!(
            "Warehouse history synchronization failed; retaining progress and exact pending page for retry: {error}\n"
        ),
    )]));
    Ok(Some(state))
}

/// An outer absence means shutdown; an inner absence is a failed read, never an empty table.
async fn read_metadata_page(
    (options, script): (&LocationServe, &Path),
    request: Vec<u8>,
    stopped: impl Future<Output = Result<(), ControlError>>,
) -> Result<Option<Option<WarehouseHistoryPage>>, ControlError> {
    let (mut child, reply) = match location_result::<Option<WarehouseHistoryPage>>(
        (options, script),
        (request, METADATA_REPLY_BYTES),
    ) {
        Ok(exchange) => exchange,
        Err(_) => return Ok(Some(None)),
    };
    tokio::pin!(stopped);
    let outcome = tokio::select! {
        result = &mut stopped => { child.kill().await?; result?; return Ok(None); },
        result = tokio::time::timeout(Duration::from_secs(METADATA_POLL_SECONDS), async {
            let page = reply.await?;
            if !child.wait().await?.success() {
                return Err(ControlError::Invalid("location metadata interpreter failed".into()));
            }
            Ok(page)
        }) => result,
    };
    if child.try_wait()?.is_none() {
        child.kill().await?;
    }
    let page = match outcome {
        Ok(Ok(page)) => page,
        Ok(Err(_)) | Err(_) => None,
    };
    Ok(Some(page))
}
