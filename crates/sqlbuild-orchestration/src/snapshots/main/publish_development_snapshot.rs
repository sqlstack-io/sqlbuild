//! Publication keeps development safety defaults inside the snapshot transaction.

use crate::snapshots::main::_publish_snapshot::publish_snapshot_with_defaults;
use crate::snapshots::models::{SnapshotInput, SnapshotRecord};
use crate::store::errors::ControlError;
use crate::store::models::ControlStore;

pub(crate) async fn publish_development_snapshot(
    store: &ControlStore,
    input: &SnapshotInput,
) -> Result<SnapshotRecord, ControlError> {
    publish_snapshot_with_defaults(store, input, true).await
}
