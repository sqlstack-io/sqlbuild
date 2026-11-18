//! Isolated read-only pages and exact acknowledgement retries share the published input fence.

use std::future::Future;
use std::path::Path;

use sqlbuild_orchestration::platform::models::{LocationWorkClient, LocationWorkPoll};
use sqlbuild_orchestration::store::errors::ControlError;
use sqlbuild_orchestration::warehouse_metadata::models::WarehouseHistoryKind;
use tokio::time::Instant;

use crate::native_commands::_helpers::command::paths::location_files;
use crate::native_commands::_helpers::location::execution::{
    begin_after_run, finish_metadata_page, freeze_history_page, load_watermark, next_scan,
    read_metadata_page, statistics_page,
};
use crate::native_commands::_helpers::location::sync_outcomes::{
    deliver_metadata_page, failed_read, retry_metadata,
};
use crate::native_commands::_helpers::location::sync_report::{metadata_health, resumed_line};
use crate::native_commands::constants::METADATA_PAGE_SECONDS;
use crate::native_commands::models::{LocationMetadataSync, LocationServe, LocationWatch};

/// One bounded source or mirror page leaves named work polling a turn between pages.
pub(crate) async fn sync_location_metadata(
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
            Ok(true) if halted => resumed_line(),
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
        let Some((read, page)) = read_metadata_page(
            (options, script, METADATA_PAGE_SECONDS),
            state,
            stopped.as_mut(),
        )
        .await?
        else {
            return Ok(None);
        };
        state = read;
        let page = match page {
            Ok(page) => page,
            Err(failure) => {
                return failed_read(state, (client, poll), (started, failure), stopped.as_mut())
                    .await;
            }
        };
        if location_files(&options.project_dir, &watch.rules, &watch.local_files)
            != watch.observed.0
        {
            return Ok(Some(state));
        }
        if page.unchanged {
            state.skip_unchanged();
            tokio::select! {
                result = &mut stopped => { result?; return Ok(None); },
                () = metadata_health(&state, (client, poll), (started, None)) => {},
            }
            return finish_metadata_page(next_scan(state)).await;
        }
        state = freeze_history_page(state, page, poll)?;
    }
    deliver_metadata_page(state, &options.target, (client, poll), (started, stopped)).await
}
