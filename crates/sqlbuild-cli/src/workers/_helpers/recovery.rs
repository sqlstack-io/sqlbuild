//! A new leader resumes supervising pinned containers without relaunching their execution.

use std::collections::BTreeSet;
use std::io::Write;

use sqlbuild_orchestration::runs::main::list_run_claims::list_run_claims;
use sqlbuild_orchestration::scheduler::main::renew_lease::renew_lease;
use sqlbuild_orchestration::scheduler::models::SchedulerLease;
use sqlbuild_orchestration::store::errors::ControlError;
use sqlbuild_orchestration::store::models::ControlStore;

#[cfg(any(target_os = "linux", target_os = "macos", windows))]
use sqlbuild_process_ownership::ownership::main::recover_worker::recover_worker;

use crate::workers::models::{ClaimedWorker, DockerWorker, Launcher, WorkerChild, Workers};

impl Workers {
    pub(in crate::workers) async fn recover(
        &mut self,
        store: &ControlStore,
        lease: &SchedulerLease,
    ) -> Result<(), ControlError> {
        if self.recovered_epoch == Some(lease.epoch) {
            return Ok(());
        }
        let mut known: BTreeSet<_> = self
            .active
            .iter()
            .map(|worker| worker.claim.claim_id)
            .collect();
        for location in self.configuration.locations() {
            #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
            if matches!(location, Launcher::Local(_)) {
                continue;
            }
            let scope = location.scope();
            let mut after = None;
            loop {
                renew_lease(store, lease).await?;
                let claims =
                    list_run_claims(store, (scope.location_id, scope.target), after).await?;
                let Some(last) = claims.last() else {
                    break;
                };
                after = Some(last.request.run_id);
                for claim in claims {
                    if !known.insert(claim.claim_id) {
                        continue;
                    }
                    let child = match location {
                        #[cfg(any(target_os = "linux", target_os = "macos", windows))]
                        Launcher::Local(location) => Some(WorkerChild::RecoveredLocal(
                            recover_worker(&location.claim(&claim))?,
                        )),
                        #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
                        Launcher::Local(_) => continue,
                        Launcher::Docker(location) => match DockerWorker::prepare(location, &claim)
                        {
                            Ok(child) => Some(WorkerChild::Docker(child)),
                            Err(ControlError::Invalid(_)) => None,
                            Err(error) => return Err(error),
                        },
                    };
                    drop(writeln!(
                        std::io::stderr().lock(),
                        "Worker supervision recovered for run {}",
                        claim.request.run_id
                    ));
                    let rejected = child.is_none();
                    let index = self.active.len();
                    self.active.push(ClaimedWorker {
                        claim,
                        child,
                        stopping: None,
                        exit: None,
                        launch_failure: rejected.then_some("docker_configuration_rejected"),
                    });
                    if rejected {
                        self.active[index]
                            .publish_launch_failure(store, lease)
                            .await?;
                    }
                }
            }
        }
        self.recovered_epoch = Some(lease.epoch);
        Ok(())
    }
}
