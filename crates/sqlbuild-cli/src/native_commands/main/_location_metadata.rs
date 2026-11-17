//! Isolated read-only pages and exact acknowledgement retries share the published input fence.

use std::future::Future;
use std::path::Path;
use std::time::Duration;

use sqlbuild_orchestration::api::errors::ClientError;
use sqlbuild_orchestration::platform::models::{
    LocationWorkClient, LocationWorkPoll, WarehouseSyncFailure,
};
use sqlbuild_orchestration::store::errors::ControlError;
use sqlbuild_orchestration::warehouse_metadata::models::WarehouseHistoryKind;
use tokio::time::Instant;

use crate::argv::models::Stream;
use crate::native_commands::_helpers::execution::{
    acknowledge_metadata_page, begin_after_run, close_metadata_reader, finish_metadata_page,
    freeze_history_page, load_watermark, location_warning, metadata_health, next_scan, page_limit,
    read_metadata_page, report_metadata_health, statistics_page,
};
use crate::native_commands::_helpers::paths::location_files;
use crate::native_commands::_helpers::terminal::write_output;
use crate::native_commands::constants::{METADATA_FAILURE_LIMIT, METADATA_POLL_SECONDS};
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
        let halted: bool = state.halted;
        let (next, due) = tokio::select! {
            result = &mut stopped => { result?; return Ok(None); },
            result = begin_after_run(state, (client, poll), &options.target) => result,
        };
        state = next;
        match due {
            Ok(true) if halted => resumed_line(state.failed_reads > 0),
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
            Ok(page) => {
                state.reads_wake_compute = page.wakes_compute;
                page
            }
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

/// A busy warehouse file waits a poll interval. Any other failed read reports its cause and
/// error, then shrinks the page after a time-out or a read too large for memory, and otherwise
/// backs off; failures in a row stop the sync until the next run, a request or a publication.
async fn failed_read(
    mut state: LocationMetadataSync,
    failure: ReadFailure,
    (client, poll): (&LocationWorkClient, &LocationWorkPoll),
    started: Instant,
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

/// A stopped synchronization that resumes says so before its first read; after a request its
/// failed reads count from zero again, otherwise one more failure stops it.
fn resumed_line(one_attempt: bool) {
    drop(write_output(&[(
        Stream::Stderr,
        "Retrying the stopped warehouse history synchronization.\n",
    )]));
    location_warning(
        "warehouse sync retrying",
        if one_attempt {
            "one attempt; another failed read stops it again"
        } else {
            "on request; failed reads back off again before they stop it"
        }
        .to_owned(),
    );
}

/// What failed, on which table and page size, and why.
pub(crate) fn read_failure_line(state: &LocationMetadataSync, failure: &ReadFailure) -> String {
    let unit = if state.kind() == WarehouseHistoryKind::Statistics {
        "relation"
    } else {
        "row"
    };
    format!(
        "Warehouse history read failed for {} ({}-{unit} page): {}.",
        state.kind().as_str().replace('_', " "),
        page_limit(state),
        failure.reason(),
    )
}
