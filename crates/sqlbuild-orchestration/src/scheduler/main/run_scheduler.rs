//! Embedded and standalone roles share the same interruptible, durable scheduling loop.

use tokio::sync::watch;

use crate::scheduler::main::scheduler_cycle::scheduler_cycle;
use crate::scheduler::models::SchedulerState;
use crate::scheduler::types::RunLauncher;
use crate::store::errors::ControlError;
use crate::store::models::ControlStore;

/// Notifications shorten the wait; durable database state, not receipt of a notification, drives work.
pub async fn run_scheduler(
    store: &ControlStore,
    mut state: SchedulerState,
    (mut shutdown, mut changes): (watch::Receiver<bool>, watch::Receiver<()>),
    mut launcher: impl RunLauncher,
) -> Result<(), ControlError> {
    let mut last_lease = None;
    let result = async {
        while !*shutdown.borrow() {
            let (next_state, cycle) = scheduler_cycle(store, state).await?;
            state = next_state;
            last_lease = state.lease.clone();
            launcher.poll(store, state.lease.as_ref()).await?;
            tokio::select! {
                _ = tokio::time::sleep(cycle.wait_for.min(std::time::Duration::from_secs(1))) => {},
                result = changes.changed() => { if result.is_err() { break; } },
                result = shutdown.changed() => { if result.is_err() { break; } },
            }
        }
        Ok(())
    }
    .await;
    let stopped = launcher.shutdown(store, last_lease.as_ref()).await;
    result.and(stopped)
}
