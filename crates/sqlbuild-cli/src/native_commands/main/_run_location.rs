//! Coalesce source edits and retain the last good publication across isolated compiler failures.

use std::future::Future;
use std::path::Path;
use std::time::Duration;
use sqlbuild_cache::project_snapshot::models::PathStamp;
use sqlbuild_orchestration::platform::main::connect_location_work::connect_location_work;
use sqlbuild_orchestration::platform::models::{LocationWorkClient, LocationWorkPoll};
use sqlbuild_orchestration::store::errors::ControlError;
use sqlbuild_orchestration::warehouse_metadata::models::WarehouseStateLocation;
use tokio::sync::{mpsc, watch};
use tokio::time::Instant;
use uuid::Uuid;
use crate::argv::models::Stream;
use crate::native_commands::_helpers::execution::{
    apply_dev_controls, compile_next_generation, finish_compile,
};
use crate::native_commands::_helpers::execution::{
    dev_controls, location_clock, location_heartbeats, next_turn, shutdown_signal,
};
use crate::native_commands::_helpers::paths::{
    exclude_location_databases, location_watch, observe_location_inputs,
};
use crate::native_commands::_helpers::terminal::write_output;
use crate::native_commands::main::_location_compiler::run_location_compiler;
use crate::native_commands::main::_location_metadata::sync_location_metadata;
use crate::native_commands::main::_location_work::run_location_work;
use crate::native_commands::models::{
    DevControlState, LocationCompileRequest, LocationMetadataSync, LocationServe, LocationWatch,
};
use crate::native_commands::types::CompileOutcome;
use sqlbuild_orchestration::platform::models::{
    ConnectionCheckTrigger, LocationHealth, LocationWorkClient, LocationWorkPoll,
};
use tokio::sync::mpsc;
use crate::native_commands::_helpers::execution::{
    check_published_location, close_metadata_reader, compile_generation, location_clock,
    location_generation, location_heartbeats, metadata_sync, next_turn, report_location_health,
    shutdown_signal,
};
use crate::native_commands::constants::LOCATION_WORK_IDLE_POLL;
use crate::native_commands::main::_location_changed_build::report_changed_build;
use crate::native_commands::main::_location_work::{next_work_check, run_location_work};
use crate::native_commands::models::{
    LocationMetadataSync, LocationRuntimeRequest, LocationServe, LocationWatch,
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
    let controls = dev_controls(&options);
    let serving = async {
        tokio::select! {
            result = watch_location(&options, script, (bearer, &client, &poll), (requests, controls)) => result,
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
<<<<<<< ours
    requests: mpsc::Sender<LocationRuntimeRequest>,
) -> Result<(), ControlError> {
    let mut watch = location_watch(&options.project_dir);
    let mut work_at = Instant::now() + LOCATION_WORK_IDLE_POLL;
=======
    (requests, mut controls): (
        mpsc::Sender<LocationCompileRequest>,
        watch::Receiver<DevControlState>,
    ),
) -> Result<(), ControlError> {
    let mut watch = location_watch(&options.project_dir);
    let mut recompiles: u64 = 0;
    let mut compiled_inputs: Option<Vec<PathStamp>> = None;
    let mut deferred_sync: Option<(Vec<WarehouseStateLocation>, i64)> = None;
    let mut work_at = Instant::now() + Duration::from_secs(2);
>>>>>>> theirs
    let mut published_generation = None;
    // The first connection check of this process is its startup check.
    let mut checked = false;
    let mut metadata = None;
    let mut clock = location_clock();
    let stopped = shutdown_signal();
    tokio::pin!(stopped);
    drop(write_output(&[(
        Stream::Stderr,
        "Starting code location; watching project inputs...\n",
    )]));
    loop {
        if let Some(result) = next_turn(
            &mut clock,
            (published_generation, &watch, &metadata),
            stopped.as_mut(),
        )
        .await
        {
            return result;
        }
        let Some(applied) = apply_dev_controls(
            (options, script, (bearer, poll.session_id)),
            (*controls.borrow_and_update(), recompiles),
            (watch, metadata, deferred_sync),
            stopped.as_mut(),
        )
        .await?
        else {
            return Ok(());
        };
        (recompiles, (watch, metadata, deferred_sync)) = applied;
        let ready;
        (watch, ready) = observe_location_inputs(&options.project_dir, watch);
        if !ready {
            let connected: bool = controls.borrow().warehouse == Some(true);
            let Some(idle) = poll_location(
<<<<<<< ours
                (options, script, &requests),
=======
                (options, script, connected),
>>>>>>> theirs
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
        let Some(next) = compile_next_generation(
            (options, bearer, &requests),
            (metadata, compiled_inputs, &watch.observed.0),
            stopped.as_mut(),
        )
        .await?
        else {
            return Ok(());
        };
        let next_compile: Option<(CompileOutcome, i64)>;
        (next_compile, metadata, compiled_inputs) = next;
        let Some((compiled, generation)) = next_compile else {
            watch.retry_at = Some(Instant::now() + Duration::from_secs(5));
            continue;
        };
<<<<<<< ours
        drop(write_output(&[(
            Stream::Stderr,
            &format!("Compiling location generation {generation} in an isolated interpreter...\n"),
        )]));
        let Some(load) =
            compile_generation(&requests, generation, &watch.observed.0, stopped.as_mut()).await?
        else {
            return Ok(());
        };
        if let Ok((refresh_at, locations)) = load {
            watch.retry_at = refresh_at;
            published_generation = Some(generation);
            let mut sync = metadata_sync(
                (locations, generation),
                (
                    options.metadata_reconcile_seconds,
                    options.safety_sync_seconds,
                ),
            );
            sync.safety_explicit = options.safety_sync_explicit;
            metadata = Some(sync);
            let trigger = if checked {
                ConnectionCheckTrigger::Publish
            } else {
                ConnectionCheckTrigger::Startup
            };
            let Some(files) = check_published_location(
                options,
                script,
                (bearer, poll.session_id, generation, trigger),
                stopped.as_mut(),
            )
            .await?
            else {
                return Ok(());
            };
            checked = true;
            watch = exclude_location_databases(&options.project_dir, files, watch);
            // The sync that follows a publication plans what changed once it completes; with no
            // state locations no sync follows, so plan now.
            if metadata
                .as_ref()
                .is_some_and(|sync| sync.locations.is_empty())
                && report_changed_build(
                    (options, &requests, &watch),
                    (client, poll.session_id, generation),
                    stopped.as_mut(),
                )
                .await?
                .is_none()
            {
                return Ok(());
            }
        } else if let Err(rules) = load {
            watch.retry_at = Some(Instant::now() + Duration::from_secs(30));
            tokio::select! {
                result = &mut stopped => return result,
                () = report_location_health(options, bearer, poll.session_id, LocationHealth::CompileFailed { generation, rules }) => {},
            }
            drop(write_output(&[(
                Stream::Stderr,
                "Location reload failed; retaining last good snapshot. Retrying on change or in 30 seconds.\n",
            )]));
        }
=======
        published_generation = compiled.0.then_some(generation);
        let Some(settled) = finish_compile(
            (options, script, (bearer, poll.session_id)),
            (compiled, generation),
            (watch, metadata, deferred_sync),
            (&mut controls, stopped.as_mut()),
        )
        .await?
        else {
            return Ok(());
        };
        (watch, metadata, deferred_sync) = settled;
>>>>>>> theirs
    }
}

/// Named work and metadata share one idle turn without transferring interpreter ownership.
async fn poll_location(
<<<<<<< ours
    (options, script, runtime): (&LocationServe, &Path, &mpsc::Sender<LocationRuntimeRequest>),
=======
    (options, script, connected): (&LocationServe, &Path, bool),
>>>>>>> theirs
    (client, poll, generation): (&LocationWorkClient, &LocationWorkPoll, Option<i64>),
    (mut watch, mut work_at, mut metadata): (LocationWatch, Instant, Option<LocationMetadataSync>),
    stopped: impl Future<Output = Result<(), ControlError>>,
) -> Result<Option<(LocationWatch, Instant, Option<LocationMetadataSync>)>, ControlError> {
    tokio::pin!(stopped);
    if generation.is_some() && watch.attempted.as_ref() == Some(&watch.observed) {
        if Instant::now() >= work_at {
<<<<<<< ours
            let Some(turn) = run_location_work(
                (options, script, runtime),
=======
            let Some((files, sync)) = run_location_work(
                (options, script, connected),
>>>>>>> theirs
                (client, poll, &watch),
                generation,
                stopped.as_mut(),
            )
            .await?
            else {
                return Ok(None);
            };
            work_at = next_work_check(&turn, Instant::now());
            watch = exclude_location_databases(&options.project_dir, turn.files, watch);
            if turn.sync
                && let Some(state) = metadata.as_mut()
            {
                state.request_sync();
            }
        }
        if let Some(state) = metadata.take() {
            let syncing = !state.idle;
            metadata = sync_location_metadata(
                state,
                (options, script, &watch),
                (client, poll),
                stopped.as_mut(),
            )
            .await?;
            let Some(state) = metadata.as_mut() else {
                return Ok(None);
            };
            if syncing && state.idle && state.take_replan() {
                let Some(reported) = report_changed_build(
                    (options, runtime, &watch),
                    (client, poll.session_id, state.generation),
                    stopped.as_mut(),
                )
                .await?
                else {
                    return Ok(None);
                };
                state.planned(reported);
            }
        }
    }
    Ok(Some((watch, work_at, metadata)))
}
