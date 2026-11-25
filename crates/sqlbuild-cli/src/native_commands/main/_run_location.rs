//! Coalesce source edits and retain the last good publication across isolated compiler failures.

use sqlbuild_orchestration::platform::models::LocationWorkScope;
use std::future::Future;
use std::path::Path;

use sqlbuild_cache::project_snapshot::models::PathStamp;
use sqlbuild_orchestration::platform::main::work::connect_location_work::connect_location_work;
use sqlbuild_orchestration::platform::models::{LocationWorkClient, LocationWorkPoll};
use sqlbuild_orchestration::store::errors::ControlError;
use sqlbuild_orchestration::warehouse_metadata::models::WarehouseStateLocation;
use tokio::sync::{mpsc, watch};
use tokio::time::Instant;
use uuid::Uuid;

use crate::argv::models::Stream;
use crate::native_commands::_helpers::command::paths::{
    exclude_location_databases, location_watch, observe_location_inputs,
};
use crate::native_commands::_helpers::command::terminal::write_output;
use crate::native_commands::_helpers::location::compile_turn::compile_turn;
use crate::native_commands::_helpers::location::execution::apply_dev_controls;
use crate::native_commands::_helpers::location::execution::{
    dev_controls, location_clock, location_heartbeats, next_turn,
};
use crate::native_commands::_helpers::location::republish::start_republish;
use crate::native_commands::_helpers::location::shutdown::shutdown_signal;
use crate::native_commands::_helpers::location::work::next_work_check;
use crate::native_commands::constants::LOCATION_WORK_IDLE_POLL;
use crate::native_commands::main::_location_changed_build::report_changed_build;
use crate::native_commands::main::_location_compiler::run_location_compiler;
use crate::native_commands::main::_location_metadata::sync_location_metadata;
use crate::native_commands::main::_location_work::run_location_work;
use crate::native_commands::models::{
    DevControlState, LocationMetadataSync, LocationRuntimeRequest, LocationServe, LocationWatch,
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
        scope: LocationWorkScope::All,
    };
    let (requests, receiver) = mpsc::channel(1);
    let (republisher, controls) = dev_controls(&options);
    let serving = async {
        tokio::select! {
            result = watch_location(&options, script, (bearer, &client, &poll), (requests, controls, &republisher)) => result,
            result = location_heartbeats(&options, bearer, session_id) => result,
        }
    };
    let (serving, compiler) = tokio::join!(
        serving,
        run_location_compiler(
            &options,
            script,
            receiver,
            (&republisher, (bearer, session_id))
        )
    );
    serving.and(compiler)
}

async fn watch_location(
    options: &LocationServe,
    script: &Path,
    (bearer, client, poll): (&str, &LocationWorkClient, &LocationWorkPoll),
    (requests, mut controls, republisher): (
        mpsc::Sender<LocationRuntimeRequest>,
        watch::Receiver<DevControlState>,
        &watch::Sender<DevControlState>,
    ),
) -> Result<(), ControlError> {
    let mut watch = location_watch(&options.project_dir);
    let mut recompiles: u64 = 0;
    let mut compiled_inputs: Option<Vec<PathStamp>> = None;
    let mut deferred_sync: Option<(Vec<WarehouseStateLocation>, i64)> = None;
    let mut work_at = Instant::now() + LOCATION_WORK_IDLE_POLL;
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
                (options, script, (&requests, connected)),
                (client, poll, (published_generation, republisher)),
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
        let Some(turn) = compile_turn(
            ((options, script), (bearer, client, poll), &requests),
            ((watch, metadata, deferred_sync), compiled_inputs),
            (&mut controls, stopped.as_mut()),
        )
        .await?
        else {
            return Ok(());
        };
        (
            ((watch, metadata, deferred_sync), compiled_inputs),
            published_generation,
        ) = turn;
    }
}

/// Named work and metadata share one idle turn; what changed is planned after a sync completes.
async fn poll_location(
    (options, script, (runtime, connected)): (
        &LocationServe,
        &Path,
        (&mpsc::Sender<LocationRuntimeRequest>, bool),
    ),
    (client, poll, (generation, republisher)): (
        &LocationWorkClient,
        &LocationWorkPoll,
        (Option<i64>, &watch::Sender<DevControlState>),
    ),
    (mut watch, mut work_at, mut metadata): (LocationWatch, Instant, Option<LocationMetadataSync>),
    stopped: impl Future<Output = Result<(), ControlError>>,
) -> Result<Option<(LocationWatch, Instant, Option<LocationMetadataSync>)>, ControlError> {
    tokio::pin!(stopped);
    let current: bool = watch.attempted.as_ref() == Some(&watch.observed);
    if current {
        if Instant::now() >= work_at {
            let poll = LocationWorkPoll {
                scope: match generation {
                    Some(_) => LocationWorkScope::All,
                    None => LocationWorkScope::RepublishOnly,
                },
                ..poll.clone()
            };
            let Some(turn) = run_location_work(
                (options, script, (runtime, connected)),
                (client, &poll, &watch),
                generation,
                stopped.as_mut(),
            )
            .await?
            else {
                return Ok(None);
            };
            work_at = next_work_check(&turn, Instant::now());
            watch = start_republish(republisher, turn.republish, watch);
            watch = exclude_location_databases(&options.project_dir, turn.files, watch);
            if turn.sync
                && let Some(state) = metadata.as_mut()
            {
                state.request_sync();
            }
        }
        if generation.is_some()
            && let Some(state) = metadata.take()
        {
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
            let unsynced: bool = state.locations.is_empty();
            let completed: bool = syncing && state.idle && !state.halted;
            if (completed || unsynced) && state.take_replan() {
                let Some(reported) = report_changed_build(
                    (options, runtime, &watch),
                    (client, poll.session_id, state.generation),
                    stopped.as_mut(),
                )
                .await?
                else {
                    return Ok(None);
                };
                if !unsynced {
                    state.planned(reported);
                }
            }
        }
    }
    Ok(Some((watch, work_at, metadata)))
}
