//! Isolated read-only pages and exact acknowledgement retries share the published input fence.

use std::future::Future;
use std::path::Path;
use std::time::Duration;

use sqlbuild_orchestration::api::errors::ClientError;
use sqlbuild_orchestration::platform::models::{
    LocationWorkClient, LocationWorkPoll, WarehouseSyncFailure,
};
use sqlbuild_orchestration::store::errors::ControlError;
use sqlbuild_orchestration::warehouse_metadata::models::{
    WarehouseHistoryKind, WarehouseHistoryPage, WarehouseWatermarkRead,
};
use tokio::time::Instant;

use crate::argv::models::Stream;
use crate::native_commands::_helpers::execution::{
    acknowledge_metadata_page, freeze_history_page, history_request, location_result,
    metadata_health, statistics_page,
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
    let started = Instant::now();
    tokio::pin!(stopped);
    if state.pending.is_none() {
        if state.kind() == WarehouseHistoryKind::Statistics && state.statistics.is_none() {
            state.statistics = Some(tokio::select! {
                result = &mut stopped => { result?; return Ok(None); },
                result = statistics_page((client, &options.target), (&state, poll)) => match result {
                    Ok(page) => page,
                    Err(error) => return retry_metadata(state, error),
                },
            });
        }
        if !state.watermark_loaded {
            if state.index == 0 {
                state.reconcile = Instant::now() >= state.reconcile_at;
            }
            if !state.reconcile
                && !matches!(
                    state.kind(),
                    WarehouseHistoryKind::Catalog | WarehouseHistoryKind::Statistics
                )
            {
                let request = WarehouseWatermarkRead {
                    session_id: poll.session_id,
                    generation: state.generation,
                    location: state.location().clone(),
                    kind: state.kind(),
                };
                let watermark = tokio::select! {
                    result = &mut stopped => { result?; return Ok(None); },
                    result = client.metadata_watermark(&options.target, &request) => result,
                };
                let watermark = match watermark {
                    Ok(watermark) => watermark,
                    Err(error) => {
                        tokio::select! {
                            result = &mut stopped => { result?; return Ok(None); },
                            () = metadata_health(&state, (client, poll), (started, Some(WarehouseSyncFailure::Watermark))) => {},
                        }
                        return retry_metadata(state, error);
                    }
                };
                state.observed_since = watermark.and_then(|time| {
                    time.checked_sub_signed(chrono::Duration::seconds(METADATA_OVERLAP_SECONDS))
                });
            }
            state.watermark_loaded = true;
        }
        let Some(page) = read_metadata_page((options, script), &state, stopped.as_mut()).await?
        else {
            return Ok(None);
        };
        let Some(page) = page else {
            tokio::select! {
                result = &mut stopped => { result?; return Ok(None); },
                () = metadata_health(&state, (client, poll), (started, Some(WarehouseSyncFailure::Read))) => {},
            }
            state.retry_read();
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
        response = client.sync_metadata(&options.target, &pending.batch) => response,
    };
    if let Err(error) = response {
        tokio::select! {
            result = &mut stopped => { result?; return Ok(None); },
            () = metadata_health(&state, (client, poll), (started, Some(WarehouseSyncFailure::Delivery))) => {},
        }
        return retry_metadata(state, error);
    }
    tokio::select! {
        result = &mut stopped => { result?; return Ok(None); },
        () = metadata_health(&state, (client, poll), (started, None)) => {},
    }
    Ok(Some(acknowledge_metadata_page(state)))
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
    state: &LocationMetadataSync,
    stopped: impl Future<Output = Result<(), ControlError>>,
) -> Result<Option<Option<WarehouseHistoryPage>>, ControlError> {
    if let Some(statistics) = &state.statistics
        && statistics.names.is_empty()
    {
        return Ok(Some(Some(WarehouseHistoryPage {
            records: Vec::new(),
            next_after: statistics.next_after.map(|id| vec![Some(id.to_string())]),
        })));
    }
    let (mut child, reply) = match location_result::<Option<WarehouseHistoryPage>>(
        (options, script),
        (history_request(state)?, METADATA_REPLY_BYTES),
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
