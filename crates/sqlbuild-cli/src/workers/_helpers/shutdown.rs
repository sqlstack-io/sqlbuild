//! Keep leadership alive while reaping children, then release only successfully reported claims.

use std::io::Write;
use std::time::{Duration, Instant};

use sqlbuild_orchestration::runs::main::observe_run::observe_run;
use sqlbuild_orchestration::runs::models::{RunChange, RunOutcome};
use sqlbuild_orchestration::scheduler::constants::RENEW_INTERVAL_SECONDS;
use sqlbuild_orchestration::scheduler::main::renew_lease::renew_lease;
use sqlbuild_orchestration::scheduler::models::SchedulerLease;
use sqlbuild_orchestration::store::errors::ControlError;
use sqlbuild_orchestration::store::models::ControlStore;

use crate::workers::_helpers::supervisor::{publish, terminal};
use crate::workers::models::{ClaimedWorker, WorkerChild, WorkerExit, Workers};

impl ClaimedWorker {
    async fn reap(
        &mut self,
        store: &ControlStore,
        lease: Option<&SchedulerLease>,
    ) -> Result<(), ControlError> {
        let Some(child) = self.child.as_mut() else {
            return Ok(());
        };
        if self.exit.is_some() {
            return Ok(());
        }
        let mut killed = false;
        if self.stopping.is_none() {
            if child.terminate().await.is_err() {
                child.kill().await?;
                killed = true;
            }
            self.stopping = Some(Instant::now());
        }
        let deadline = self.stopping.unwrap_or_else(Instant::now) + Duration::from_secs(60);
        let mut renew = tokio::time::interval(Duration::from_secs(RENEW_INTERVAL_SECONDS));
        loop {
            tokio::select! {
                exit = child.wait() => {
                    self.exit = Some(exit?);
                    return Ok(());
                }
                _ = tokio::time::sleep_until(deadline.into()), if !killed => {
                    child.kill().await?;
                    killed = true;
                }
                _ = renew.tick(), if lease.is_some() => {
                    if let Some(lease) = lease
                        && renew_lease(store, lease).await.is_err()
                    {
                        drop(writeln!(std::io::stderr().lock(), "Scheduler lease renewal failed during worker shutdown"));
                    }
                }
            }
        }
    }

    async fn finish_shutdown(
        &mut self,
        store: &ControlStore,
        lease: Option<&SchedulerLease>,
    ) -> Result<(), ControlError> {
        self.reap(store, lease).await?;
        let state = observe_run(store, self.claim.request.run_id).await?;
        if !terminal(state.status) {
            let lease = lease.ok_or(ControlError::Fenced)?;
            let reason = if self.child.is_none() {
                "worker launch preparation failed before a child started"
            } else {
                "scheduler confirmed its worker stopped or absent"
            };
            publish(
                store,
                lease,
                &self.claim,
                (
                    "local-shutdown",
                    RunChange::Finished {
                        outcome: if state.cancel_requested_at.is_some() {
                            RunOutcome::Cancelled
                        } else {
                            RunOutcome::Failed
                        },
                        reason: reason.into(),
                        exit_code: self.exit.and_then(WorkerExit::code),
                    },
                ),
            )
            .await?;
        }
        Ok(())
    }
}

impl Workers {
    pub(in crate::workers) async fn shutdown_workers(
        &mut self,
        store: &ControlStore,
        lease: Option<&SchedulerLease>,
    ) -> Result<(), ControlError> {
        let mut retrying = false;
        let mut renew = tokio::time::interval(Duration::from_secs(RENEW_INTERVAL_SECONDS));
        let mut renewed = Instant::now();
        while !self.active.is_empty() {
            let mut failure = None;
            for index in (0..self.active.len()).rev() {
                if renewed.elapsed() >= Duration::from_secs(RENEW_INTERVAL_SECONDS)
                    && let Some(lease) = lease
                {
                    if let Err(error) = renew_lease(store, lease).await {
                        failure = Some(error);
                    }
                    renewed = Instant::now();
                }
                if let Some(WorkerChild::Docker(child)) = &self.active[index].child {
                    match child.try_wait().await {
                        Ok(None) => {
                            self.active.swap_remove(index);
                            continue;
                        }
                        Ok(exit) => self.active[index].exit = exit,
                        Err(error) => {
                            failure.get_or_insert(error);
                            continue;
                        }
                    }
                }
                match self.active[index].finish_shutdown(store, lease).await {
                    Ok(()) => {
                        self.active.swap_remove(index);
                    }
                    Err(ControlError::Fenced) => failure = Some(ControlError::Fenced),
                    Err(error) => {
                        failure.get_or_insert(error);
                    }
                }
            }
            if matches!(failure, Some(ControlError::Fenced)) {
                return Err(ControlError::Fenced);
            }
            if failure.is_some() {
                if !retrying {
                    drop(writeln!(
                        std::io::stderr().lock(),
                        "Retrying worker shutdown publication or physical observation; claims remain owned"
                    ));
                    retrying = true;
                }
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs(1)) => {},
                    _ = renew.tick(), if lease.is_some() => {
                        if let Some(lease) = lease
                            && matches!(renew_lease(store, lease).await, Err(ControlError::Fenced))
                        {
                            return Err(ControlError::Fenced);
                        }
                    }
                }
            }
        }
        if retrying {
            drop(writeln!(
                std::io::stderr().lock(),
                "Worker shutdown recovered; stopped claims released and live containers remain recoverable"
            ));
        }
        Ok(())
    }
}
