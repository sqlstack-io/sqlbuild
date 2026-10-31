//! Drain a bounded due-trigger page, renewing the lease between persisted evaluations.

use uuid::Uuid;

use crate::scheduler::main::evaluate_trigger::evaluate_trigger;
use crate::scheduler::main::renew_lease::renew_lease;
use crate::scheduler::models::{SchedulerCycle, SchedulerLease};
use crate::store::errors::ControlError;
use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::main::query::query;
use crate::store::models::{ControlStore, Parameter, TransactionMode};

pub(crate) async fn drain_due_triggers(
    store: &ControlStore,
    mut lease: SchedulerLease,
    mut cycle: SchedulerCycle,
) -> Result<(SchedulerLease, SchedulerCycle), ControlError> {
    let transaction = begin(store, TransactionMode::Read).await?;
    let now = transaction.now;
    let (transaction, rows) = query(transaction,
        "SELECT s.trigger_id FROM sqb_trigger_state s JOIN sqb_triggers t ON t.trigger_id=s.trigger_id \
         JOIN sqb_locations l ON l.location_id=t.location_id \
         WHERE s.status='running' AND s.next_due_at<=$1 AND t.removed_in_snapshot_id IS NULL AND l.archived_at IS NULL \
         ORDER BY s.next_due_at, s.trigger_id LIMIT 100", vec![Parameter::Time(now)]).await?;
    commit(transaction).await?;
    for row in rows {
        let trigger_id: Uuid = serde_json::from_value(row["trigger_id"].clone())
            .map_err(|error| ControlError::Invalid(error.to_string()))?;
        if let Some(plan) = evaluate_trigger(store, &lease, trigger_id).await? {
            cycle.evaluated += 1;
            cycle.ticks += plan.actions.len();
        }
        lease = renew_lease(store, &lease).await?;
    }
    Ok((lease, cycle))
}
