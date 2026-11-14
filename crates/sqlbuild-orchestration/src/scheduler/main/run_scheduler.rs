//! Embedded and standalone roles share the same interruptible, durable scheduling loop.

use std::time::Duration;

use tokio::sync::watch;
use tokio::time::Instant;

use crate::scheduler::main::_scheduler_cycle::scheduler_cycle;
use crate::scheduler::models::SchedulerState;
use crate::scheduler::types::RunLauncher;
use crate::store::errors::ControlError;
use crate::store::models::ControlStore;

/// Durable database state drives work; a transient failure or a lost lease retries the next cycle.
pub async fn run_scheduler(
    store: &ControlStore,
    mut state: SchedulerState,
    mut shutdown: watch::Receiver<bool>,
    mut launcher: impl RunLauncher,
) -> Result<(), ControlError> {
    let mut last_lease = None;
    let result = async {
        while !*shutdown.borrow() {
            let polled = match scheduler_cycle(store, state.clone()).await {
                Ok((next_state, cycle)) => {
                    state = next_state;
                    last_lease = state.lease.clone();
                    launcher.cycled(&cycle);
                    launcher
                        .poll(store, state.lease.as_ref())
                        .await
                        .map(|()| Instant::from_std(cycle.wait_from + cycle.wait_for))
                }
                Err(error) => Err(error),
            };
            let retry = Instant::now() + Duration::from_secs(1);
            let due = match polled {
                Ok(due) => due.min(retry),
                Err(error) if error.transient() || matches!(error, ControlError::Fenced) => {
                    launcher.retrying(&error);
                    retry
                }
                Err(error) => return Err(error),
            };
            tokio::select! {
                _ = tokio::time::sleep_until(due) => {},
                result = shutdown.changed() => { if result.is_err() { break; } },
            }
        }
        Ok(())
    }
    .await;
    let stopped = launcher.shutdown(store, last_lease.as_ref()).await;
    result.and(stopped)
}
