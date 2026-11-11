//! Embedded and standalone roles share the same interruptible, durable scheduling loop.

use std::time::Duration;

use tokio::sync::watch;

use crate::scheduler::main::_scheduler_cycle::scheduler_cycle;
use crate::scheduler::models::SchedulerState;
use crate::scheduler::types::RunLauncher;
use crate::store::errors::ControlError;
use crate::store::models::ControlStore;

/// Durable database state drives work; a transient store failure retries the next cycle.
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
                    launcher
                        .poll(store, state.lease.as_ref())
                        .await
                        .map(|()| cycle.wait_for)
                }
                Err(error) => Err(error),
            };
            let wait_for = match polled {
                Ok(wait_for) => wait_for,
                Err(error) if error.transient() => {
                    launcher.retrying(&error);
                    Duration::from_secs(1)
                }
                Err(error) => return Err(error),
            };
            tokio::select! {
                _ = tokio::time::sleep(wait_for.min(Duration::from_secs(1))) => {},
                result = shutdown.changed() => { if result.is_err() { break; } },
            }
        }
        Ok(())
    }
    .await;
    let stopped = launcher.shutdown(store, last_lease.as_ref()).await;
    result.and(stopped)
}
