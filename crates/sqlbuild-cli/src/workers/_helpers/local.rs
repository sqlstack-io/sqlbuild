//! The privileged control role owns and reaps local children; workers receive claim-scoped HTTP.

use std::io::Write;
use std::process::{ExitStatus, Stdio};
use std::time::{Duration, Instant};

use chrono::{TimeDelta, Utc};
use sqlbuild_core::command_output::constants::INVOCATION_CONTEXT_ENV;
use sqlbuild_orchestration::credentials::main::issue_token::issue_token;
use sqlbuild_orchestration::credentials::models::{AuditContext, CredentialScope};
use sqlbuild_orchestration::runs::main::change_run::change_run;
use sqlbuild_orchestration::runs::main::claim_next_run::claim_next_run;
use sqlbuild_orchestration::runs::main::observe_run::observe_run;
use sqlbuild_orchestration::runs::models::{
    RunAuthority, RunChange, RunChangeRequest, RunClaim, RunOutcome, RunStatus,
};
use sqlbuild_orchestration::scheduler::models::SchedulerLease;
use sqlbuild_orchestration::store::errors::ControlError;
use sqlbuild_orchestration::store::models::ControlStore;
use tokio::process::{Child, Command};
use uuid::Uuid;

use crate::workers::models::{LocalLauncher, LocalWorker, LocalWorkers};

impl sqlbuild_orchestration::scheduler::types::RunLauncher for LocalWorkers {
    async fn poll(
        &mut self,
        store: &ControlStore,
        lease: Option<&SchedulerLease>,
    ) -> Result<(), ControlError> {
        for index in (0..self.active.len()).rev() {
            let worker = &mut self.active[index];
            let mut state = observe_run(store, worker.claim.request.run_id).await?;
            if let Some(child) = worker.child.as_mut() {
                worker.exit = worker.exit.or(child.try_wait()?);
            }
            if worker.exit.is_none()
                && let Some(child) = worker.child.as_mut()
            {
                let now = state.observed_at;
                let not_started = matches!(state.status, RunStatus::Queued | RunStatus::Launched)
                    && state
                        .claimed_at
                        .is_some_and(|claimed| now - claimed > TimeDelta::minutes(2));
                let silent = state
                    .last_heartbeat_at
                    .is_some_and(|last| now - last > TimeDelta::minutes(10));
                if (state.cancel_requested_at.is_some() || not_started || silent)
                    && worker.stopping.is_none()
                {
                    terminate(child)?;
                    worker.stopping = Some(Instant::now());
                }
                if worker
                    .stopping
                    .is_some_and(|at| at.elapsed() >= Duration::from_secs(60))
                {
                    child.kill().await?;
                    worker.exit = child.try_wait()?;
                    state = observe_run(store, worker.claim.request.run_id).await?;
                }
            }
            if worker.exit.is_some() || worker.child.is_none() {
                if !terminal(state.status) {
                    let Some(lease) = lease else {
                        continue;
                    };
                    let outcome = if state.cancel_requested_at.is_some() {
                        RunOutcome::Cancelled
                    } else {
                        RunOutcome::Failed
                    };
                    publish(
                        store,
                        lease,
                        &worker.claim,
                        (
                            "local-exit",
                            RunChange::Finished {
                                outcome,
                                reason: "local worker exited; physical child has been reaped"
                                    .into(),
                                exit_code: worker.exit.map(exit_code),
                            },
                        ),
                    )
                    .await?;
                }
                self.active.swap_remove(index);
            }
        }
        let Some(lease) = lease else {
            return Ok(());
        };
        for location in &self.configuration.local {
            let running = self
                .active
                .iter()
                .filter(|worker| {
                    worker.claim.request.location_id == location.location_id
                        && worker.claim.request.target == location.target
                })
                .count();
            for _ in running..location.max_workers.get() {
                let Some(claim) = claim_next_run(
                    store,
                    lease,
                    &self.configuration.concurrency_limits,
                    Some((location.location_id, &location.target)),
                )
                .await?
                else {
                    break;
                };
                let index = self.active.len();
                self.active.push(LocalWorker {
                    claim,
                    child: None,
                    stopping: None,
                    exit: None,
                });
                let worker = &mut self.active[index];
                let claim = &worker.claim;
                if claim.request.image_id.is_some() {
                    publish(store, lease, claim, ("local-image-rejected", RunChange::Finished { outcome: RunOutcome::Failed, reason: "image-pinned runs require a Docker launcher, not a local interpreter".into(), exit_code: None })).await?;
                    self.active.swap_remove(index);
                    continue;
                }
                let context = AuditContext {
                    actor: format!("scheduler/{}", lease.holder),
                    granting_role: "scheduler".into(),
                    request_id: claim.claim_id,
                    ip_address: None,
                };
                let credential = issue_token(
                    store,
                    &context,
                    CredentialScope::Run {
                        run_id: claim.request.run_id,
                        claim_id: claim.claim_id,
                    },
                    Utc::now() + TimeDelta::days(7),
                )
                .await?;
                publish(
                    store,
                    lease,
                    claim,
                    ("local-launched", RunChange::Launched { container_id: None }),
                )
                .await?;
                match spawn(location, claim, &credential.bearer, &self.control_variable) {
                    Ok(child) => {
                        drop(writeln!(
                            std::io::stderr().lock(),
                            "Worker launched for run {}",
                            claim.request.run_id
                        ));
                        worker.child = Some(child);
                    }
                    Err(_) => {
                        publish(
                            store,
                            lease,
                            claim,
                            (
                                "local-spawn-failed",
                                RunChange::Finished {
                                    outcome: RunOutcome::Failed,
                                    reason: "local worker process could not start".into(),
                                    exit_code: None,
                                },
                            ),
                        )
                        .await?;
                        self.active.swap_remove(index);
                    }
                }
            }
        }
        Ok(())
    }

    /// Never release claims until children have physically stopped, even if shutdown loses its fence.
    async fn shutdown(
        &mut self,
        store: &ControlStore,
        lease: Option<&SchedulerLease>,
    ) -> Result<(), ControlError> {
        self.shutdown_workers(store, lease).await
    }
}

fn spawn(
    location: &LocalLauncher,
    claim: &RunClaim,
    bearer: &str,
    control_variable: &str,
) -> Result<Child, ControlError> {
    Ok(Command::new(&location.executable)
        .args([
            "location",
            "execute",
            "--server",
            &location.server_url,
            "--location-id",
            &location.location_id.to_string(),
            "--target",
            &location.target,
            "--run-id",
            &claim.request.run_id.to_string(),
            "--claim-id",
            &claim.claim_id.to_string(),
            "--project-dir",
        ])
        .arg(&location.project_dir)
        .current_dir(&location.project_dir)
        .env_remove(control_variable)
        .env_remove("SQLBUILD_CONTROL_DATABASE_URL")
        .env_remove("SQLBUILD_LOCATION_TOKEN")
        .env("SQLBUILD_RUN_TOKEN", bearer)
        .env(
            INVOCATION_CONTEXT_ENV,
            serde_json::json!({
                "run_id": claim.request.run_id,
                "integration": {
                    "name": "sqlbuild",
                    "run_id": claim.request.run_id,
                    "job_name": claim.request.job_key,
                },
            })
            .to_string(),
        )
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .spawn()?)
}

pub(in crate::workers) async fn publish(
    store: &ControlStore,
    lease: &SchedulerLease,
    claim: &RunClaim,
    (event, change): (&str, RunChange),
) -> Result<(), ControlError> {
    change_run(
        store,
        RunAuthority::Scheduler(lease),
        &RunChangeRequest {
            event_id: Uuid::new_v5(&claim.claim_id, event.as_bytes()),
            run_id: claim.request.run_id,
            claim_id: Some(claim.claim_id),
            change,
        },
    )
    .await?;
    Ok(())
}

pub(in crate::workers) fn terminal(status: RunStatus) -> bool {
    !matches!(
        status,
        RunStatus::Queued | RunStatus::Launched | RunStatus::Running | RunStatus::Unresponsive
    )
}

pub(in crate::workers) fn exit_code(exit: ExitStatus) -> i64 {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        i64::from(
            exit.code()
                .unwrap_or_else(|| 128 + exit.signal().unwrap_or(0)),
        )
    }
    #[cfg(not(unix))]
    i64::from(exit.code().unwrap_or(1))
}

pub(in crate::workers) fn terminate(child: &mut Child) -> Result<(), ControlError> {
    #[cfg(unix)]
    if let Some(id) = child.id() {
        let raw = i32::try_from(id)
            .map_err(|error| ControlError::Invalid(format!("invalid child process ID: {error}")))?;
        let pid = rustix::process::Pid::from_raw(raw)
            .ok_or_else(|| ControlError::Invalid("invalid child process ID".into()))?;
        match rustix::process::kill_process(pid, rustix::process::Signal::TERM) {
            Ok(()) | Err(rustix::io::Errno::SRCH) => {}
            Err(error) => return Err(std::io::Error::from(error).into()),
        }
    }
    #[cfg(not(unix))]
    let _ = child;
    Ok(())
}
