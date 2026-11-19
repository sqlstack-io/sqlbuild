//! Named outbound work shares the watcher's generation boundary, never its interpreter.

use std::future::Future;
use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::Utc;
use sqlbuild_orchestration::api::errors::ClientError;
use sqlbuild_orchestration::platform::models::{
    LocationWork, LocationWorkClient, LocationWorkOutcome, LocationWorkPoll, LocationWorkResult,
};
use sqlbuild_orchestration::store::errors::ControlError;
use tokio::sync::mpsc;
use tokio::time::Instant;
use uuid::Uuid;

use crate::argv::models::Stream;
use crate::native_commands::_helpers::command::paths::location_files;
use crate::native_commands::_helpers::command::terminal::write_output;
use crate::native_commands::_helpers::location::execution::check_location_connection;
use crate::native_commands::_helpers::location::work::work_error;
use crate::native_commands::main::_location_plan::plan_location;
use crate::native_commands::main::_location_sensor::evaluate_location_sensor;
use crate::native_commands::models::{
    LocationRuntimeRequest, LocationServe, LocationWatch, LocationWorkTurn,
};

/// Retry the exact completed result without rerunning it; a turn's `sync` asks the metadata loop to sync.
pub(super) async fn run_location_work(
    (options, script, (runtime, connected)): (
        &LocationServe,
        &Path,
        (&mpsc::Sender<LocationRuntimeRequest>, bool),
    ),
    (client, poll, watch): (&LocationWorkClient, &LocationWorkPoll, &LocationWatch),
    generation: Option<i64>,
    stopped: impl Future<Output = Result<(), ControlError>>,
) -> Result<Option<LocationWorkTurn>, ControlError> {
    tokio::pin!(stopped);
    let claim = tokio::select! {
        result = &mut stopped => { result?; return Ok(None); },
        result = client.poll(poll) => match result {
            Ok(claim) => claim,
            Err(error) => {
                work_error(&error)?;
                return Ok(Some(LocationWorkTurn::default()));
            },
        },
    };
    let Some(claim) = claim else {
        return Ok(Some(LocationWorkTurn::default()));
    };
    let deadline = Instant::now()
        + (claim.lease_expires_at - Utc::now())
            .to_std()
            .unwrap_or_default();
    let mut files: Vec<PathBuf> = Vec::new();
    let outcome = if generation == Some(claim.generation)
        && location_files(&options.project_dir, &watch.rules, &watch.local_files)
            == watch.observed.0
    {
        let mut shutting_down = false;
        let expires = async {
            tokio::select! {
                result = &mut stopped => { shutting_down = true; result },
                () = tokio::time::sleep_until(deadline) => Ok(()),
            }
        };
        let copy = (client, poll.session_id, claim.generation);
        let checked = perform_work(
            (options, script, (runtime, connected)),
            claim.request.work,
            copy,
            expires,
        )
        .await?;
        let Some((outcome, local_files)) = checked else {
            drop(write_output(&[(
                Stream::Stderr,
                "Location work stopped before completion; no result reported.\n",
            )]));
            return Ok((!shutting_down).then_some(LocationWorkTurn {
                files,
                sync: false,
                claimed: true,
            }));
        };
        files = local_files;
        let mut expected = watch.observed.clone();
        expected
            .0
            .retain(|stamp| !files.contains(&options.project_dir.join(&stamp.relative_path)));
        let mut excluded = watch.local_files.clone();
        excluded.extend(files.clone());
        if location_files(&options.project_dir, &watch.rules, &excluded) == expected.0 {
            outcome
        } else {
            LocationWorkOutcome::Superseded {}
        }
    } else {
        LocationWorkOutcome::Superseded {}
    };
    let sync = matches!(outcome, LocationWorkOutcome::SyncStarted {});
    let result = LocationWorkResult {
        claim_id: claim.claim_id,
        session_id: poll.session_id,
        outcome,
    };
    while Instant::now() < deadline {
        let completed = tokio::select! {
            result = &mut stopped => { result?; return Ok(None); },
            () = tokio::time::sleep_until(deadline) => break,
            result = client.complete(claim.request.request_id, &result) => result,
        };
        match completed {
            Ok(_) => {
                drop(write_output(&[(
                    Stream::Stderr,
                    "Location work completed.\n",
                )]));
                return Ok(Some(LocationWorkTurn {
                    files,
                    sync,
                    claimed: true,
                }));
            }
            Err(error) => {
                work_error(&error)?;
                if matches!(error, ClientError::Rejected(status) if status.is_client_error()) {
                    break;
                }
            }
        }
        tokio::select! {
            result = &mut stopped => { result?; return Ok(None); },
            () = tokio::time::sleep(Duration::from_secs(1)) => {},
        }
    }
    drop(write_output(&[(
        Stream::Stderr,
        "Location work acknowledgement expired; check the server result before retrying.\n",
    )]));
    Ok(Some(LocationWorkTurn {
        files,
        sync,
        claimed: true,
    }))
}

/// One named operation; `None` means it stopped at shutdown or its lease ended; `sqb dev` never signs in here.
async fn perform_work(
    (options, script, (runtime, connected)): (
        &LocationServe,
        &Path,
        (&mpsc::Sender<LocationRuntimeRequest>, bool),
    ),
    work: LocationWork,
    copy: (&LocationWorkClient, Uuid, i64),
    expires: impl Future<Output = Result<(), ControlError>>,
) -> Result<Option<(LocationWorkOutcome, Vec<PathBuf>)>, ControlError> {
    let message = match &work {
        LocationWork::CheckConnection {} => "Checking requested location connection...\n",
        LocationWork::SyncWarehouse {} => "Synchronizing warehouse history on request...\n",
        LocationWork::EvaluateSensor { .. } => "Evaluating requested custom sensor...\n",
        LocationWork::Plan { .. } => "Planning requested command...\n",
    };
    drop(write_output(&[(Stream::Stderr, message)]));
    Ok(match work {
        LocationWork::SyncWarehouse {} => Some((LocationWorkOutcome::SyncStarted {}, Vec::new())),
        LocationWork::CheckConnection {} => {
            check_location_connection((options, script, connected), expires)
                .await?
                .map(|(healthy, _, files)| (LocationWorkOutcome::Connection { healthy }, files))
        }
        LocationWork::EvaluateSensor { evaluation } => {
            evaluate_location_sensor(options, script, &evaluation, expires)
                .await?
                .map(|outcome| {
                    (
                        LocationWorkOutcome::SensorEvaluation { outcome },
                        Vec::new(),
                    )
                })
        }
        LocationWork::Plan {
            selection,
            settings,
            fresh,
            ..
        } => plan_location(
            (options, runtime),
            (&selection, &settings, fresh),
            copy,
            expires,
        )
        .await?
        .map(|(document, diffs)| (LocationWorkOutcome::Plan { document, diffs }, Vec::new())),
    })
}
