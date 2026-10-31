//! Every server observes health independently; scheduler leadership is not required.

use std::time::Duration;

use tokio::sync::watch;
use uuid::Uuid;

use crate::platform::_helpers::watchdog::scan_health;
use crate::store::errors::ControlError;
use crate::store::models::ControlStore;

pub async fn run_watchdog(
    store: &ControlStore,
    replica_id: Uuid,
    mut shutdown: watch::Receiver<bool>,
) -> Result<(), ControlError> {
    while !*shutdown.borrow() {
        scan_health(store, replica_id, &shutdown).await?;
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs(30)) => {},
            result = shutdown.changed() => { if result.is_err() { break; } },
        }
    }
    Ok(())
}
