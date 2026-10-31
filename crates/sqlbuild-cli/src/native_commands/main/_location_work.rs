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
use tokio::time::Instant;

use crate::argv::models::Stream;
use crate::native_commands::_helpers::execution::check_location_connection;
use crate::native_commands::_helpers::paths::location_inputs;
use crate::native_commands::_helpers::terminal::write_output;
use crate::native_commands::models::{LocationServe, LocationWatch};

/// Retry the exact completed result after uncertain delivery; never rerun it to obtain a new result.
pub(super) async fn run_location_work(
    (options, script): (&LocationServe, &Path),
    (client, poll, watch): (&LocationWorkClient, &LocationWorkPoll, &LocationWatch),
    generation: Option<i64>,
    stopped: impl Future<Output = Result<(), ControlError>>,
) -> Result<Option<Vec<PathBuf>>, ControlError> {
    tokio::pin!(stopped);
    let claim = tokio::select! {
        result = &mut stopped => { result?; return Ok(None); },
        result = client.poll(poll) => match result {
            Ok(claim) => claim,
            Err(error) => {
                work_error(&error)?;
                return Ok(Some(Vec::new()));
            },
        },
    };
    let Some(claim) = claim else {
        return Ok(Some(Vec::new()));
    };
    let deadline = Instant::now()
        + (claim.lease_expires_at - Utc::now())
            .to_std()
            .unwrap_or_default();
    let mut files: Vec<PathBuf> = Vec::new();
    let outcome = if generation == Some(claim.generation) {
        drop(write_output(&[(
            Stream::Stderr,
            "Checking requested location connection...\n",
        )]));
        let mut shutting_down = false;
        let expires = async {
            tokio::select! {
                result = &mut stopped => { shutting_down = true; result },
                () = tokio::time::sleep_until(deadline) => Ok(()),
            }
        };
        let checked = match claim.request.work {
            LocationWork::CheckConnection {} => {
                check_location_connection(options, script, expires).await?
            }
        };
        let Some((healthy, local_files)) = checked else {
            drop(write_output(&[(
                Stream::Stderr,
                "Location work stopped before completion; no result reported.\n",
            )]));
            return Ok((!shutting_down).then_some(files));
        };
        files = local_files;
        let mut expected = watch.observed.clone();
        expected
            .0
            .retain(|stamp| !files.contains(&options.project_dir.join(&stamp.relative_path)));
        let mut excluded = watch.local_files.clone();
        excluded.extend(files.clone());
        if location_inputs(&options.project_dir, &watch.rules, &excluded) == expected {
            LocationWorkOutcome::Connection { healthy }
        } else {
            LocationWorkOutcome::Superseded {}
        }
    } else {
        LocationWorkOutcome::Superseded {}
    };
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
                return Ok(Some(files));
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
    Ok(Some(files))
}

fn work_error(error: &ClientError) -> Result<(), ControlError> {
    if matches!(error, ClientError::Configuration(_))
        || matches!(error, ClientError::Rejected(status) if matches!(status.as_u16(), 401 | 403))
    {
        return Err(ControlError::Invalid(error.to_string()));
    }
    drop(write_output(&[(
        Stream::Stderr,
        &format!("Location work request failed: {error}\n"),
    )]));
    Ok(())
}
