//! A new leader resumes supervising pinned containers without relaunching their execution.

use std::collections::BTreeSet;
use std::io::Write;

use sqlbuild_orchestration::runs::main::list_run_claims::list_run_claims;
use sqlbuild_orchestration::scheduler::main::renew_lease::renew_lease;
use sqlbuild_orchestration::scheduler::models::SchedulerLease;
use sqlbuild_orchestration::store::errors::ControlError;
use sqlbuild_orchestration::store::models::ControlStore;

use crate::workers::models::{ClaimedWorker, DockerWorker, WorkerChild, Workers};

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
        for location in &self.configuration.docker {
            let mut after = None;
            loop {
                renew_lease(store, lease).await?;
                let claims =
                    list_run_claims(store, (location.location_id, &location.target), after).await?;
                let Some(last) = claims.last() else {
                    break;
                };
                after = Some(last.request.run_id);
                for claim in claims {
                    if !known.insert(claim.claim_id) {
                        continue;
                    }
                    let child = match DockerWorker::prepare(location, &claim) {
                        Ok(child) => Some(WorkerChild::Docker(child)),
                        Err(ControlError::Invalid(_)) => None,
                        Err(error) => return Err(error),
                    };
                    drop(writeln!(
                        std::io::stderr().lock(),
                        "Worker supervision recovered for run {}",
                        claim.request.run_id
                    ));
                    self.active.push(ClaimedWorker {
                        claim,
                        child,
                        stopping: None,
                        exit: None,
                    });
                }
            }
        }
        self.recovered_epoch = Some(lease.epoch);
        Ok(())
    }
}
