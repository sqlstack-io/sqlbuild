//! Embedded and standalone roles share the same interruptible, durable scheduling loop.

use tokio::sync::watch;

use crate::scheduler::main::scheduler_cycle::scheduler_cycle;
use crate::scheduler::models::SchedulerState;
use crate::store::errors::ControlError;
use crate::store::models::ControlStore;

/// Notifications shorten the wait; durable database state, not receipt of a notification, drives work.
pub async fn run_scheduler(
    store: &ControlStore,
    mut state: SchedulerState,
    mut shutdown: watch::Receiver<bool>,
    mut changes: watch::Receiver<()>,
) -> Result<(), ControlError> {
    while !*shutdown.borrow() {
        let (next_state, cycle) = scheduler_cycle(store, state).await?;
        state = next_state;
        tokio::select! {
            _ = tokio::time::sleep(cycle.wait_for) => {},
            result = changes.changed() => { if result.is_err() { break; } },
            result = shutdown.changed() => { if result.is_err() { break; } },
        }
    }
    Ok(())
}
