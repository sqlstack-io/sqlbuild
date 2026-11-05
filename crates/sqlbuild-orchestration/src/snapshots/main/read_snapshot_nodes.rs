//! Read retained node definitions without expanding their display-only source bodies.

use std::collections::{BTreeMap, BTreeSet};

use uuid::Uuid;

use crate::snapshots::_helpers::reading::{query_bodies, read_nodes};
use crate::snapshots::models::SnapshotNode;
use crate::store::errors::ControlError;
use crate::store::models::ControlTransaction;

pub(crate) async fn read_snapshot_nodes(
    transaction: ControlTransaction,
    snapshot_id: Uuid,
    selection: Option<&BTreeSet<String>>,
) -> Result<(ControlTransaction, BTreeMap<String, SnapshotNode>), ControlError> {
    let (transaction, rows) = query_bodies(
        transaction,
        (
            "SELECT n.node_key, d.body FROM sqb_snapshot_nodes n JOIN sqb_node_definitions d \
             ON d.node_hash=n.node_hash WHERE n.snapshot_id=$1",
            "n.node_key",
        ),
        snapshot_id,
        selection,
    )
    .await?;
    Ok((transaction, read_nodes(rows)?))
}
