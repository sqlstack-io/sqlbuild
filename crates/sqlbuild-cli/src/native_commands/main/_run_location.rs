//! Coalesce source edits and retain the last good publication across isolated compiler failures.

use std::future::Future;
use std::path::Path;
use std::time::Duration;

use sqlbuild_orchestration::platform::main::connect_location_work::connect_location_work;
use sqlbuild_orchestration::platform::models::{
    LocationHealth, LocationWorkClient, LocationWorkPoll,
};
use sqlbuild_orchestration::store::errors::ControlError;
use tokio::sync::mpsc;
use tokio::time::{Instant, MissedTickBehavior};
use uuid::Uuid;

use crate::argv::models::Stream;
use crate::native_commands::_helpers::execution::{
    check_published_location, close_metadata_reader, compile_generation, location_generation,
    location_heartbeats, metadata_sync, report_location_health, shutdown_signal,
};
use crate::native_commands::_helpers::paths::{
    exclude_location_databases, location_watch, observe_location_inputs,
};
use crate::native_commands::_helpers::terminal::write_output;
use crate::native_commands::main::_location_compiler::run_location_compiler;
use crate::native_commands::main::_location_metadata::sync_location_metadata;
use crate::native_commands::main::_location_work::run_location_work;
use crate::native_commands::models::{
    LocationCompileRequest, LocationMetadataSync, LocationServe, LocationWatch,
};

pub(super) async fn run_location(
    options: LocationServe,
    script: &Path,
    bearer: &str,
) -> Result<(), ControlError> {
    let session_id = Uuid::now_v7();
    let client = connect_location_work((&options.server_url, bearer), options.location_id)
        .map_err(|error| ControlError::Invalid(error.to_string()))?;
    let poll = LocationWorkPoll {
        session_id,
        target: options.target.clone(),
    };
    let (requests, receiver) = mpsc::channel(1);
    let serving = async {
        tokio::select! {
            result = watch_location(&options, script, (bearer, &client, &poll), requests) => result,
            result = location_heartbeats(&options, bearer, session_id) => result,
        }
    };
    let (serving, compiler) =
        tokio::join!(serving, run_location_compiler(&options, script, receiver));
    serving.and(compiler)
}

async fn watch_location(
    options: &LocationServe,
    script: &Path,
    (bearer, client, poll): (&str, &LocationWorkClient, &LocationWorkPoll),
    requests: mpsc::Sender<LocationCompileRequest>,
) -> Result<(), ControlError> {
    let mut watch = location_watch(&options.project_dir);
    let mut work_at = Instant::now() + Duration::from_secs(2);
    let mut published_generation = None;
    let mut metadata = None;
    let mut clock = tokio::time::interval(Duration::from_millis(250));
    clock.set_missed_tick_behavior(MissedTickBehavior::Skip);
    let stopped = shutdown_signal();
    tokio::pin!(stopped);
    drop(write_output(&[(
        Stream::Stderr,
        "Starting code location; watching project inputs...\n",
    )]));
    loop {
        tokio::select! {
            result = &mut stopped => return result,
            _ = clock.tick() => {},
        }
        let ready;
        (watch, ready) = observe_location_inputs(&options.project_dir, watch);
        if !ready {
            let Some(idle) = poll_location(
                (options, script),
                (client, poll, published_generation),
                (watch, work_at, metadata),
                stopped.as_mut(),
            )
            .await?
            else {
                return Ok(());
            };
            (watch, work_at, metadata) = idle;
            continue;
        }
        published_generation = None;
        if let Some(state) = metadata.take() {
            metadata = Some(close_metadata_reader(state).await?);
        }
        let generation = tokio::select! {
            result = &mut stopped => return result,
            generation = location_generation(options, bearer) => generation?,
        };
        let generation = match generation {
            Some(generation) => generation,
            None => {
                watch.retry_at = Some(Instant::now() + Duration::from_secs(5));
                continue;
            }
        };
        drop(write_output(&[(
            Stream::Stderr,
            &format!("Compiling location generation {generation} in an isolated interpreter...\n"),
        )]));
        let Some((published, refresh_at, locations)) =
            compile_generation(&requests, generation, &watch.observed.0, stopped.as_mut()).await?
        else {
            return Ok(());
        };
        if published {
            watch.retry_at = refresh_at;
            published_generation = Some(generation);
            metadata = Some(metadata_sync(
                (locations, generation),
                (
                    options.metadata_reconcile_seconds,
                    options.safety_sync_seconds,
                ),
            ));
            let Some(files) = check_published_location(
                options,
                script,
                (bearer, poll.session_id, generation),
                stopped.as_mut(),
            )
            .await?
            else {
                return Ok(());
            };
            watch = exclude_location_databases(&options.project_dir, files, watch);
        } else {
            watch.retry_at = Some(Instant::now() + Duration::from_secs(30));
            tokio::select! {
                result = &mut stopped => return result,
                () = report_location_health(options, bearer, poll.session_id, LocationHealth::CompileFailed { generation }) => {},
            }
            drop(write_output(&[(
                Stream::Stderr,
                "Location reload failed; retaining last good snapshot. Retrying on change or in 30 seconds.\n",
            )]));
        }
    }
}

/// Named work and metadata share one idle turn without transferring interpreter ownership.
async fn poll_location(
    (options, script): (&LocationServe, &Path),
    (client, poll, generation): (&LocationWorkClient, &LocationWorkPoll, Option<i64>),
    (mut watch, mut work_at, mut metadata): (LocationWatch, Instant, Option<LocationMetadataSync>),
    stopped: impl Future<Output = Result<(), ControlError>>,
) -> Result<Option<(LocationWatch, Instant, Option<LocationMetadataSync>)>, ControlError> {
    tokio::pin!(stopped);
    if generation.is_some() && watch.attempted.as_ref() == Some(&watch.observed) {
        if Instant::now() >= work_at {
            let Some((files, sync)) = run_location_work(
                (options, script),
                (client, poll, &watch),
                generation,
                stopped.as_mut(),
            )
            .await?
            else {
                return Ok(None);
            };
            watch = exclude_location_databases(&options.project_dir, files, watch);
            work_at = Instant::now() + Duration::from_secs(2);
            if sync && let Some(state) = metadata.as_mut() {
                state.request_sync();
            }
        }
        if let Some(state) = metadata.take() {
            metadata = sync_location_metadata(
                state,
                (options, script, &watch),
                (client, poll),
                stopped.as_mut(),
            )
            .await?;
            if metadata.is_none() {
                return Ok(None);
            }
        }
    }
    Ok(Some((watch, work_at, metadata)))
}
