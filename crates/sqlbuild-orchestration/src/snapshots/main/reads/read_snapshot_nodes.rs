//! Read the listed nodes' definitions without expanding their display-only source bodies; whole snapshots go through `snapshot_nodes`.

use std::collections::{BTreeMap, BTreeSet};

use uuid::Uuid;

use crate::snapshots::_helpers::reading::{query_bodies, read_nodes};
use crate::snapshots::models::SnapshotNode;
use crate::store::errors::ControlError;
use crate::store::models::ControlTransaction;

/// The listed nodes of the snapshot, decoded off the async workers; an empty list reads none.
pub(crate) async fn read_snapshot_nodes(
    transaction: ControlTransaction,
    snapshot_id: Uuid,
    selection: &BTreeSet<String>,
) -> Result<(ControlTransaction, BTreeMap<String, SnapshotNode>), ControlError> {
    let select = format!(
        "SELECT n.node_key, {} AS body FROM sqb_snapshot_nodes n JOIN sqb_node_definitions d \
         ON d.node_hash=n.node_hash WHERE n.snapshot_id=$1",
        transaction.hex_column("d.body"),
    );
    let (transaction, rows) = query_bodies(
        transaction,
        (&select, "n.node_key"),
        snapshot_id,
        Some(selection),
    )
    .await?;
    let nodes = tokio::task::spawn_blocking(move || read_nodes(rows)).await??;
    Ok((transaction, nodes))
}
