//! One privileged owner supervises both launchers; workers receive only claim-scoped HTTP.

use std::io::Write;
use std::process::{ExitStatus, Stdio};
use std::time::{Duration, Instant};

use chrono::{TimeDelta, Utc};
use sqlbuild_orchestration::credentials::main::issue_token::issue_token;
use sqlbuild_orchestration::credentials::models::{AuditContext, CredentialScope};
use sqlbuild_orchestration::runs::main::change_run::change_run;
use sqlbuild_orchestration::runs::main::claim_next_run::claim_next_run;
use sqlbuild_orchestration::runs::main::observe_run::observe_run;
use sqlbuild_orchestration::runs::models::{
    RunAuthority, RunChange, RunChangeRequest, RunClaim, RunOutcome, RunStatus,
};
use sqlbuild_orchestration::scheduler::constants::RENEW_INTERVAL_SECONDS;
use sqlbuild_orchestration::scheduler::main::renew_lease::renew_lease;
use sqlbuild_orchestration::scheduler::models::SchedulerLease;
use sqlbuild_orchestration::store::errors::ControlError;
use sqlbuild_orchestration::store::models::ControlStore;
use tokio::process::{Child, Command};
use uuid::Uuid;

use crate::workers::_helpers::invocation::{arguments, environment, excluded};
use crate::workers::models::{
    ClaimedWorker, DockerWorker, Launcher, LocalLauncher, WorkerChild, WorkerExit, Workers,
};

impl sqlbuild_orchestration::scheduler::types::RunLauncher for Workers {
    async fn poll(
        &mut self,
        store: &ControlStore,
        lease: Option<&SchedulerLease>,
    ) -> Result<(), ControlError> {
        if let Some(lease) = lease {
            self.recover(store, lease).await?;
        }
        let mut renewed = Instant::now();
        for index in (0..self.active.len()).rev() {
            if renewed.elapsed() >= Duration::from_secs(RENEW_INTERVAL_SECONDS)
                && let Some(lease) = lease
            {
                renew_lease(store, lease).await?;
                renewed = Instant::now();
            }
            let worker = &mut self.active[index];
            let mut state = observe_run(store, worker.claim.request.run_id).await?;
            if let Some(child) = worker.child.as_mut() {
                worker.exit = worker.exit.or(child.try_wait().await?);
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
                    child.terminate().await?;
                    worker.stopping = Some(Instant::now());
                }
                if worker
                    .stopping
                    .is_some_and(|at| at.elapsed() >= Duration::from_secs(60))
                {
                    child.kill().await?;
                    worker.exit = child.try_wait().await?;
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
                                reason: "worker stopped; physical exit or absence confirmed".into(),
                                exit_code: worker.exit.and_then(WorkerExit::code),
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
        self.launch_available(store, lease).await
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

impl Workers {
    async fn launch_available(
        &mut self,
        store: &ControlStore,
        lease: &SchedulerLease,
    ) -> Result<(), ControlError> {
        let mut renewed = Instant::now();
        for location in self.configuration.locations() {
            let scope = location.scope();
            let running = self
                .active
                .iter()
                .filter(|worker| {
                    worker.claim.request.location_id == scope.location_id
                        && worker.claim.request.target == scope.target
                })
                .count();
            for _ in running..scope.max_workers.get() {
                if renewed.elapsed() >= Duration::from_secs(RENEW_INTERVAL_SECONDS) {
                    renew_lease(store, lease).await?;
                    renewed = Instant::now();
                }
                let Some(claim) = claim_next_run(
                    store,
                    lease,
                    &self.configuration.concurrency_limits,
                    Some((scope.location_id, scope.target)),
                )
                .await?
                else {
                    break;
                };
                let index = self.active.len();
                self.active.push(ClaimedWorker {
                    claim,
                    child: None,
                    stopping: None,
                    exit: None,
                });
                let worker = &mut self.active[index];
                let claim = &worker.claim;
                if matches!(location, Launcher::Local(_)) && claim.request.image_id.is_some() {
                    publish(store, lease, claim, ("local-image-rejected", RunChange::Finished { outcome: RunOutcome::Failed, reason: "image-pinned runs require a Docker launcher, not a local interpreter".into(), exit_code: None })).await?;
                    self.active.swap_remove(index);
                    continue;
                }
                let container_id = if let Launcher::Docker(location) = location {
                    match DockerWorker::prepare(location, claim) {
                        Ok(child) => {
                            let name = child.name.clone();
                            worker.child = Some(WorkerChild::Docker(child));
                            Some(name)
                        }
                        Err(_) => {
                            publish(store, lease, claim, ("docker-image-rejected", RunChange::Finished {
                                outcome: RunOutcome::Failed, reason: "Docker worker requires a pinned image and supported launcher configuration".into(), exit_code: None,
                            })).await?;
                            self.active.swap_remove(index);
                            continue;
                        }
                    }
                } else {
                    None
                };
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
                    ("local-launched", RunChange::Launched { container_id }),
                )
                .await?;
                let launched = match location {
                    Launcher::Local(location) => {
                        spawn(location, claim, &credential.bearer, &self.control_variable).map(
                            |child| {
                                worker.child = Some(WorkerChild::Local(child));
                            },
                        )
                    }
                    Launcher::Docker(location) => {
                        if let Some(WorkerChild::Docker(child)) = &worker.child {
                            child
                                .launch(location, claim, &credential.bearer, &self.control_variable)
                                .await?;
                        }
                        Ok(())
                    }
                };
                match launched {
                    Ok(()) => {
                        drop(writeln!(
                            std::io::stderr().lock(),
                            "Worker launched for run {}",
                            claim.request.run_id
                        ));
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
}

fn spawn(
    location: &LocalLauncher,
    claim: &RunClaim,
    bearer: &str,
    control_variable: &str,
) -> Result<Child, ControlError> {
    Ok(Command::new(&location.executable)
        .args(arguments(&Launcher::Local(location).scope(), claim))
        .arg(&location.project_dir)
        .current_dir(&location.project_dir)
        .env_clear()
        .envs(std::env::vars_os().filter(|(key, _)| {
            key.to_str()
                .is_none_or(|key| !excluded(key, control_variable))
        }))
        .envs(environment(claim, bearer))
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
