//! A cycle renews leadership and drains a bounded set of persisted due triggers.

use std::time::Duration;

use chrono::{DateTime, Utc};

use crate::runs::constants::MONITOR_BATCH_SIZE;
use crate::runs::main::monitor_runs::monitor_runs;
use crate::scheduler::_helpers::drain_due_triggers::drain_due_triggers;
use crate::scheduler::constants::RENEW_INTERVAL_SECONDS;
use crate::scheduler::main::acquire_lease::acquire_lease;
use crate::scheduler::main::renew_lease::renew_lease;
use crate::scheduler::models::{SchedulerCycle, SchedulerState};
use crate::store::errors::ControlError;
use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::main::query::query;
use crate::store::models::{ControlStore, TransactionMode};

pub async fn scheduler_cycle(
    store: &ControlStore,
    mut state: SchedulerState,
) -> Result<(SchedulerState, SchedulerCycle), ControlError> {
    let renewal = match state.lease.as_ref() {
        Some(lease) => renew_lease(store, lease).await.map(Some),
        None => acquire_lease(store, state.holder).await,
    };
    state.lease = match renewal {
        Ok(lease) => lease,
        Err(ControlError::Fenced) => None,
        Err(error) => return Err(error),
    };
    let cycle = SchedulerCycle {
        leader: state.lease.is_some(),
        evaluated: 0,
        ticks: 0,
        unresponsive: 0,
        wait_for: Duration::from_secs(RENEW_INTERVAL_SECONDS),
    };
    let Some(lease) = state.lease.clone() else {
        return Ok((state, cycle));
    };
    let (lease, mut cycle) = drain_due_triggers(store, lease, cycle).await?;
    cycle.unresponsive = monitor_runs(store, &lease).await?;
    if cycle.unresponsive == MONITOR_BATCH_SIZE {
        cycle.wait_for = Duration::ZERO;
    }
    state.lease = Some(lease);
    let transaction = begin(store, TransactionMode::Read).await?;
    let now = transaction.now;
    let (transaction, rows) = query(transaction,
        "SELECT MIN(s.next_due_at) AS next_due_at FROM sqb_trigger_state s JOIN sqb_triggers t ON t.trigger_id=s.trigger_id \
         JOIN sqb_locations l ON l.location_id=t.location_id \
         WHERE s.status='running' AND t.removed_in_snapshot_id IS NULL AND l.archived_at IS NULL", vec![]).await?;
    let next: Option<DateTime<Utc>> = serde_json::from_value(rows[0]["next_due_at"].clone())
        .map_err(|error| ControlError::Invalid(error.to_string()))?;
    if let Some(next) = next {
        cycle.wait_for = (next - now)
            .to_std()
            .unwrap_or(Duration::ZERO)
            .min(cycle.wait_for);
    }
    commit(transaction).await?;
    Ok((state, cycle))
}
