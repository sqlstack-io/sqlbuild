//! Publish a generation using ordinary production trigger defaults.

use crate::snapshots::main::_publish_snapshot::publish_snapshot_with_defaults;
use crate::snapshots::models::{SnapshotInput, SnapshotRecord};
use crate::store::errors::ControlError;
use crate::store::models::ControlStore;

/// Reject stale generations without changing the last good snapshot.
pub async fn publish_snapshot(
    store: &ControlStore,
    input: &SnapshotInput,
) -> Result<SnapshotRecord, ControlError> {
    publish_snapshot_with_defaults(store, input, false).await
}
