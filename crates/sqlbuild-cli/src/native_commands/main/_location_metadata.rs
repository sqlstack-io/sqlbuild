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
    MetadataKeyPage, MetadataScan, WarehouseHistoryKind, WarehouseHistoryPage,
};
use tokio::time::Instant;

use crate::argv::models::Stream;
use crate::native_commands::_helpers::execution::{
    acknowledge_metadata_page, begin_after_run, freeze_history_page, load_watermark,
    metadata_health, next_scan, statistics_page,
};
use crate::native_commands::_helpers::execution::{finish_metadata_page, read_metadata_page};
use crate::native_commands::_helpers::paths::location_inputs;
use crate::native_commands::_helpers::terminal::write_output;
use crate::native_commands::constants::METADATA_POLL_SECONDS;
use crate::native_commands::models::{LocationMetadataSync, LocationServe, LocationWatch};

/// Shutdown, a rejected key page, or the page with its missing rows (`None` when the read failed).
type MissingRows = Option<(
    LocationMetadataSync,
    Result<Option<WarehouseHistoryPage>, ClientError>,
)>;

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
            read_metadata_page((options, script), (state, None), stopped.as_mut()).await?
        else {
            return Ok(None);
        };
        state = read;
        let page = match page {
            Some(page) if !page.keys.is_empty() => {
                let Some((read, rows)) = missing_rows(
                    (state, page),
                    (options, script),
                    (client, poll),
                    stopped.as_mut(),
                )
                .await?
                else {
                    return Ok(None);
                };
                state = read;
                match rows {
                    Ok(rows) => rows,
                    Err(error) => {
                        tokio::select! {
                            result = &mut stopped => { result?; return Ok(None); },
                            () = metadata_health(&state, (client, poll), (started, Some(WarehouseSyncFailure::Delivery))) => {},
                        }
                        return retry_metadata(state, error);
                    }
                }
            }
            page => page,
        };
        let Some(page) = page else {
            let failure = if state.busy {
                WarehouseSyncFailure::Busy
            } else {
                WarehouseSyncFailure::Read
            };
            tokio::select! {
                result = &mut stopped => { result?; return Ok(None); },
                () = metadata_health(&state, (client, poll), (started, Some(failure))) => {},
            }
            if state.busy {
                state.next_at = Instant::now() + Duration::from_secs(METADATA_POLL_SECONDS);
                return Ok(Some(state));
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
        if page.unchanged {
            tokio::select! {
                result = &mut stopped => { result?; return Ok(None); },
                () = metadata_health(&state, (client, poll), (started, None)) => {},
            }
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

/// The mirror's rows join the scan by key; only the rows it lacks are read in full.
async fn missing_rows(
    (state, page): (LocationMetadataSync, WarehouseHistoryPage),
    (options, script): (&LocationServe, &Path),
    (client, poll): (&LocationWorkClient, &LocationWorkPoll),
    stopped: impl Future<Output = Result<(), ControlError>>,
) -> Result<MissingRows, ControlError> {
    tokio::pin!(stopped);
    let request = MetadataKeyPage {
        kind: state.kind(),
        session_id: poll.session_id,
        generation: state.generation,
        scan: MetadataScan {
            scan_id: state.scan_id,
            location: state.location().clone(),
            complete: false,
        },
        keys: page.keys.clone(),
    };
    let matched = tokio::select! {
        result = &mut stopped => { result?; return Ok(None); },
        result = client.match_metadata_keys(&options.target, &request) => result,
    };
    let missing: Vec<serde_json::Value> = match matched {
        Ok(matched) => matched
            .missing
            .iter()
            .filter_map(|index| request.keys.get(*index).cloned())
            .collect(),
        Err(error) => return Ok(Some((state, Err(error)))),
    };
    if missing.is_empty() {
        return Ok(Some((state, Ok(Some(page)))));
    }
    let Some((state, read)) =
        read_metadata_page((options, script), (state, Some(&missing)), stopped).await?
    else {
        return Ok(None);
    };
    let page = read.map(|read| WarehouseHistoryPage {
        records: read.records,
        ..page
    });
    Ok(Some((state, Ok(page))))
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
            "Warehouse history synchronization failed; retaining progress and exact pending page for retry: {error}\n"
        ),
    )]));
    Ok(Some(state))
}
