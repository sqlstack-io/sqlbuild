//! Isolated read-only pages and exact acknowledgement retries share the published input fence.

use std::future::Future;
use std::path::Path;
use std::time::Duration;

use sqlbuild_orchestration::api::errors::ClientError;
use sqlbuild_orchestration::platform::models::{
    LocationWorkClient, LocationWorkPoll, WarehouseReadFailure, WarehouseSyncFailure,
};
use sqlbuild_orchestration::store::errors::ControlError;
use sqlbuild_orchestration::warehouse_metadata::models::WarehouseHistoryKind;
use tokio::time::Instant;

use crate::argv::models::Stream;
use crate::native_commands::_helpers::execution::{
    acknowledge_metadata_page, begin_after_run, freeze_history_page, load_watermark,
    metadata_health, next_scan, page_limit, report_metadata_health, statistics_page,
};
use crate::native_commands::_helpers::execution::{finish_metadata_page, read_metadata_page};
use crate::native_commands::_helpers::paths::location_files;
use crate::native_commands::_helpers::terminal::write_output;
use crate::native_commands::constants::METADATA_POLL_SECONDS;
use crate::native_commands::models::{
    LocationMetadataSync, LocationServe, LocationWatch, ReadFailure,
};

/// One bounded source or mirror page leaves named work polling a turn between pages.
pub(super) async fn sync_location_metadata(
    mut state: LocationMetadataSync,
    (options, script, watch): (&LocationServe, &Path, &LocationWatch),
    (client, poll): (&LocationWorkClient, &LocationWorkPoll),
    stopped: impl Future<Output = Result<(), ControlError>>,
) -> Result<Option<LocationMetadataSync>, ControlError> {
    if Instant::now() < state.next_at
        || state.locations.is_empty()
        || location_files(&options.project_dir, &watch.rules, &watch.local_files)
            != watch.observed.0
    {
        return Ok(Some(state));
    }
    let started = Instant::now();
    tokio::pin!(stopped);
    if state.idle {
        let (next, due) = tokio::select! {
            result = &mut stopped => { result?; return Ok(None); },
            result = begin_after_run(state, (client, poll), &options.target) => result,
        };
        state = next;
        match due {
            Ok(true) => {}
            Ok(false) => return Ok(Some(state)),
            Err(error) => return retry_metadata(state, error),
        }
    }
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
            let Some((loaded, failure)) =
                load_watermark(state, (options, client, poll), (started, stopped.as_mut())).await?
            else {
                return Ok(None);
            };
            state = loaded;
            if let Some(error) = failure {
                return retry_metadata(state, error);
            }
        }
        let Some((read, page)) =
            read_metadata_page((options, script), state, stopped.as_mut()).await?
        else {
            return Ok(None);
        };
        state = read;
        let page = match page {
            Ok(page) => page,
            Err(failure) => {
                return failed_read(state, failure, (client, poll), started, stopped.as_mut())
                    .await;
            }
        };
        if location_files(&options.project_dir, &watch.rules, &watch.local_files)
            != watch.observed.0
        {
            return Ok(Some(state));
        }
        if page.unchanged {
            tokio::select! {
                result = &mut stopped => { result?; return Ok(None); },
                () = metadata_health(&state, (client, poll), (started, None)) => {},
            }
            state.progress.scan_unchanged = true;
            return finish_metadata_page(next_scan(state)).await;
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
    finish_metadata_page(acknowledge_metadata_page(state)).await
}

/// A busy warehouse file waits a poll interval; any other failed read retries a smaller page.
async fn failed_read(
    mut state: LocationMetadataSync,
    failure: ReadFailure,
    (client, poll): (&LocationWorkClient, &LocationWorkPoll),
    started: Instant,
    stopped: impl Future<Output = Result<(), ControlError>>,
) -> Result<Option<LocationMetadataSync>, ControlError> {
    let (stage, cause) = match &failure {
        ReadFailure::Busy => (WarehouseSyncFailure::Busy, None),
        ReadFailure::ReaderStart(_) => (
            WarehouseSyncFailure::Read,
            Some(WarehouseReadFailure::ReaderStart {}),
        ),
        ReadFailure::Read(_) => (
            WarehouseSyncFailure::Read,
            Some(WarehouseReadFailure::ReadError {}),
        ),
        ReadFailure::TimedOut(seconds) => (
            WarehouseSyncFailure::Read,
            Some(WarehouseReadFailure::TimedOut { seconds: *seconds }),
        ),
    };
    tokio::select! {
        result = stopped => { result?; return Ok(None); },
        () = report_metadata_health(&state, (client, poll), (started, Some(stage), cause)) => {},
    }
    if state.busy {
        state.next_at = Instant::now() + Duration::from_secs(METADATA_POLL_SECONDS);
        return Ok(Some(state));
    }
    drop(write_output(&[(
        Stream::Stderr,
        &read_failure_line(&state, &failure),
    )]));
    state.retry_read();
    Ok(Some(state))
}

/// What failed, on which table and page size, and why, before the retry shrinks the page.
pub(crate) fn read_failure_line(state: &LocationMetadataSync, failure: &ReadFailure) -> String {
    let unit = if state.kind() == WarehouseHistoryKind::Statistics {
        "relation"
    } else {
        "row"
    };
    let cause = match failure {
        ReadFailure::Busy => "another process holds the warehouse file".to_owned(),
        ReadFailure::ReaderStart(error) => format!("the metadata reader did not start: {error}"),
        ReadFailure::Read(message) => message.trim().to_owned(),
        ReadFailure::TimedOut(seconds) => format!("no reply within {seconds} s"),
    };
    format!(
        "Warehouse history read failed for {} ({}-{unit} page): {cause}. Sync incomplete; retaining mirror and retrying a smaller page.\n",
        state.kind().as_str().replace('_', " "),
        page_limit(state),
    )
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
    state.reader = None;
    drop(write_output(&[(
        Stream::Stderr,
        &format!(
            "Warehouse history sync incomplete: {error}. Retaining progress and the exact pending page for retry.\n"
        ),
    )]));
    Ok(Some(state))
}
