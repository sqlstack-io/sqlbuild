//! Terminal facts exercise claim release through the public run lifecycle.

use sqlbuild_orchestration::runs::main::change_run::change_run;
use sqlbuild_orchestration::runs::models::{
    RunAuthority, RunChange, RunChangeRequest, RunClaim, RunOutcome,
};
use sqlbuild_orchestration::scheduler::models::SchedulerLease;
use sqlbuild_orchestration::store::models::ControlStore;
use uuid::Uuid;

pub(crate) async fn finish(store: &ControlStore, lease: &SchedulerLease, claim: &RunClaim) {
    let request = RunChangeRequest {
        event_id: Uuid::new_v5(&claim.claim_id, b"test-finished"),
        run_id: claim.request.run_id,
        claim_id: Some(claim.claim_id),
        change: RunChange::Finished {
            outcome: RunOutcome::Failed,
            reason: "synthetic worker exit".into(),
            exit_code: Some(1),
        },
    };
    assert!(
        change_run(store, RunAuthority::Scheduler(lease), &request)
            .await
            .expect("terminate claimed work")
    );
    assert!(
        !change_run(store, RunAuthority::Scheduler(lease), &request)
            .await
            .expect("retry terminal fact")
    );
}
