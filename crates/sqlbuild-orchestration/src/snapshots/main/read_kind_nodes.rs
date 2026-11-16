//! Read one snapshot's nodes of the given kinds, so callers skip the definitions they never look at.

use std::collections::BTreeMap;

use uuid::Uuid;

use crate::snapshots::_helpers::reading::{hex_column, query_bodies, read_nodes};
use crate::snapshots::models::SnapshotNode;
use crate::store::errors::ControlError;
use crate::store::models::ControlTransaction;

/// The snapshot's nodes whose kind is listed; an empty list reads none.
pub(crate) async fn read_kind_nodes(
    transaction: ControlTransaction,
    snapshot_id: Uuid,
    kinds: &[&str],
) -> Result<(ControlTransaction, BTreeMap<String, SnapshotNode>), ControlError> {
    let select = format!(
        "SELECT n.node_key, {} AS body FROM sqb_snapshot_nodes n JOIN sqb_node_definitions d \
         ON d.node_hash=n.node_hash WHERE n.snapshot_id=$1",
        hex_column(&transaction, "d.body"),
    );
    let (transaction, rows) = query_bodies(
        transaction,
        (&select, "n.node_kind"),
        snapshot_id,
        Some(kinds),
    )
    .await?;
    Ok((transaction, read_nodes(rows)?))
}
