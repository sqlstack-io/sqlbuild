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
    let mut published_generation = None;
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
                (options, script, connected),
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
    }
}

/// Named work and metadata share one idle turn without transferring interpreter ownership.
async fn poll_location(
    (options, script, connected): (&LocationServe, &Path, bool),
    (client, poll, generation): (&LocationWorkClient, &LocationWorkPoll, Option<i64>),
    (mut watch, mut work_at, mut metadata): (LocationWatch, Instant, Option<LocationMetadataSync>),
    stopped: impl Future<Output = Result<(), ControlError>>,
) -> Result<Option<(LocationWatch, Instant, Option<LocationMetadataSync>)>, ControlError> {
    tokio::pin!(stopped);
    if generation.is_some() && watch.attempted.as_ref() == Some(&watch.observed) {
        if Instant::now() >= work_at {
            let Some((files, sync)) = run_location_work(
                (options, script, connected),
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
