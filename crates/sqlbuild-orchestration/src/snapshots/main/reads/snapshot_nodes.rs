//! A snapshot's every node, decoded once and shared, so concurrent readers never hold more than one copy.

use std::sync::Arc;

use uuid::Uuid;

use crate::snapshots::_helpers::whole_snapshot::{SNAPSHOT_NODES, read_budgeted, slot};
use crate::snapshots::types::SnapshotNodes;
use crate::store::errors::ControlError;
use crate::store::models::ControlStore;

/// Every node of the snapshot; the first reader decodes it within the shared memory budget while the others wait.
pub(crate) async fn snapshot_nodes(
    store: &ControlStore,
    snapshot_id: Uuid,
) -> Result<SnapshotNodes, ControlError> {
    let cell = slot(&SNAPSHOT_NODES, snapshot_id);
    let nodes = cell
        .get_or_try_init(|| async {
            let read = read_budgeted(&SNAPSHOT_NODES, store, snapshot_id);
            #[cfg(test)]
            let read = crate::store::_helpers::statement_count::STATEMENTS
                .scope(std::cell::Cell::new(0), read);
            Ok::<_, ControlError>(Arc::new(read.await?))
        })
        .await?;
    Ok(Arc::clone(nodes))
}
