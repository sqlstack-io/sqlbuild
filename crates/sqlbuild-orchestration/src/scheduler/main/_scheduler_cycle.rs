//! A cycle renews leadership and drains a bounded set of persisted due triggers.

use std::time::Duration;

use crate::runs::constants::MONITOR_BATCH_SIZE;
use crate::runs::main::monitor_runs::monitor_runs;
use crate::scheduler::_helpers::drain_due_triggers::{drain_due_triggers, wait_for_next_due};
use crate::scheduler::_helpers::tick_retention::prune_ticks_when_due;
use crate::scheduler::constants::RENEW_INTERVAL_SECONDS;
use crate::scheduler::main::_acquire_lease::acquire_lease;
use crate::scheduler::main::renew_lease::renew_lease;
use crate::scheduler::models::{SchedulerCycle, SchedulerState};
use crate::store::errors::ControlError;
use crate::store::models::ControlStore;

pub(crate) async fn scheduler_cycle(
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
    state.pruned_at = prune_ticks_when_due(store, &lease, state.pruned_at).await?;
    state.lease = Some(lease);
    cycle.wait_for = wait_for_next_due(store, cycle.wait_for).await?;
    Ok((state, cycle))
}
