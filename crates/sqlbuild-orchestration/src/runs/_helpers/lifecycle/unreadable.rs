//! Fail one run or group whose pinned snapshot can't be read, with the normal failed status and events, so scheduling carries on for everything else.

use uuid::Uuid;

use crate::runs::_helpers::groups::lifecycle::group_change;
use crate::runs::_helpers::lifecycle::change_run::change_run_in;
use crate::runs::models::{RunAuthority, RunChange, RunChangeRequest, RunOutcome};
use crate::scheduler::main::check_lease::check_lease;
use crate::scheduler::models::SchedulerLease;
use crate::store::constants::UNREADABLE_SNAPSHOT;
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{ControlStore, Parameter, TransactionMode};

/// The run's failure, as the scheduler records it.
fn failed() -> RunChange {
    RunChange::Finished {
        outcome: RunOutcome::Failed,
        reason: UNREADABLE_SNAPSHOT.into(),
        exit_code: None,
    }
}

/// End the unclaimed run `run_id` failed, in its own transaction under the scheduler's lease.
pub(crate) async fn fail_unreadable_run(
    store: &ControlStore,
    lease: &SchedulerLease,
    run_id: Uuid,
) -> Result<(), ControlError> {
    let transaction = check_lease(begin(store, TransactionMode::Write).await?, lease).await?;
    let (transaction, _) = change_run_in(
        transaction,
        RunAuthority::Scheduler(lease),
        &RunChangeRequest {
            event_id: Uuid::new_v5(&run_id, b"unreadable-snapshot"),
            run_id,
            claim_id: None,
            change: failed(),
        },
    )
    .await?;
    commit(check_lease(transaction, lease).await?).await
}

/// Ask each unfinished member of group `group_id` to cancel, then end the group failed, in one transaction under the scheduler's lease.
pub(crate) async fn fail_unreadable_group(
    store: &ControlStore,
    lease: &SchedulerLease,
    group_id: Uuid,
) -> Result<(), ControlError> {
    let mut transaction = check_lease(begin(store, TransactionMode::Write).await?, lease).await?;
    let members;
    (transaction, members) = query(
        transaction,
        "SELECT run_id FROM sqb_runs WHERE parent_run_id=$1 AND cancel_requested_at IS NULL \
         AND status IN ('queued','launched','running','unresponsive') ORDER BY run_id",
        vec![Parameter::Id(group_id)],
    )
    .await?;
    for member in members {
        let member_id: Uuid = serde_json::from_value(member["run_id"].clone())
            .map_err(|error| ControlError::Invalid(error.to_string()))?;
        transaction = group_change(
            transaction,
            member_id,
            RunChange::CancelRequested {
                actor: "system:group".into(),
                reason: UNREADABLE_SNAPSHOT.into(),
            },
        )
        .await?;
    }
    let group;
    (transaction, group) = query(
        transaction,
        "SELECT 1 AS open FROM sqb_runs WHERE run_id=$1 AND status IN ('queued','launched','running','unresponsive')",
        vec![Parameter::Id(group_id)],
    )
    .await?;
    if !group.is_empty() {
        transaction = group_change(transaction, group_id, failed()).await?;
    }
    commit(check_lease(transaction, lease).await?).await
}
