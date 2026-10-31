//! Coalesce source edits and retain the last good publication across isolated compiler failures.

use std::future::Future;
use std::path::{Path, PathBuf};
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
    check_location_connection, compile_generation, location_generation, location_heartbeats,
    report_location_health, shutdown_signal,
};
use crate::native_commands::_helpers::paths::{
    exclude_location_databases, location_watch, observe_location_inputs,
};
use crate::native_commands::_helpers::terminal::write_output;
use crate::native_commands::main::_location_compiler::run_location_compiler;
use crate::native_commands::main::_location_work::run_location_work;
use crate::native_commands::models::{LocationCompileRequest, LocationServe};

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
            if Instant::now() >= work_at
                && published_generation.is_some()
                && watch.attempted.as_ref() == Some(&watch.observed)
            {
                let Some(files) = run_location_work(
                    (options, script),
                    (client, poll, &watch),
                    published_generation,
                    stopped.as_mut(),
                )
                .await?
                else {
                    return Ok(());
                };
                watch = exclude_location_databases(&options.project_dir, files, watch);
                work_at = Instant::now() + Duration::from_secs(2);
            }
            continue;
        }
        published_generation = None;
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
        let Some(published) =
            compile_generation(&requests, generation, &watch.observed.0, stopped.as_mut()).await?
        else {
            return Ok(());
        };
        if published {
            watch.retry_at = None;
            published_generation = Some(generation);
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

async fn check_published_location(
    options: &LocationServe,
    script: &Path,
    (bearer, session_id, generation): (&str, Uuid, i64),
    stopped: impl Future<Output = Result<(), ControlError>>,
) -> Result<Option<Vec<PathBuf>>, ControlError> {
    drop(write_output(&[(
        Stream::Stderr,
        "Checking location connection with sqb debug...\n",
    )]));
    tokio::pin!(stopped);
    let Some((connected, files)) =
        check_location_connection(options, script, stopped.as_mut()).await?
    else {
        return Ok(None);
    };
    tokio::select! {
        result = &mut stopped => { result?; return Ok(None); },
        () = report_location_health(options, bearer, session_id, LocationHealth::Connection { generation, healthy: connected }) => {},
    }
    drop(write_output(&[(
        Stream::Stderr,
        if connected {
            "Code location ready; connection check passed; watching for changes.\n"
        } else {
            "Code location published; connection check failed; watching for changes.\n"
        },
    )]));
    Ok(Some(files))
}
