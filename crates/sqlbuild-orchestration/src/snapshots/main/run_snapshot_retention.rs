//! Collect unreferenced old snapshots periodically; every pass is safe to repeat.

use std::time::Duration;

use crate::snapshots::_helpers::retention::prune_snapshots;
use crate::snapshots::models::SnapshotRetention;
use crate::store::errors::ControlError;
use crate::store::models::ControlStore;

pub async fn run_snapshot_retention(
    store: &ControlStore,
    retention: SnapshotRetention,
) -> Result<(), ControlError> {
    let mut interval = tokio::time::interval(Duration::from_secs(60));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        interval.tick().await;
        match prune_snapshots(store, retention).await {
            Ok(_) => {}
            Err(ControlError::Database(error)) if ControlError::transient_database(&error) => {}
            Err(error) => return Err(error),
        }
    }
}
