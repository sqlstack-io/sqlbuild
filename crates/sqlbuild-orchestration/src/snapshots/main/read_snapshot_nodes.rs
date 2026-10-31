//! Read retained node definitions without expanding their display-only source bodies.

use std::collections::{BTreeMap, BTreeSet};

use uuid::Uuid;

use crate::snapshots::_helpers::reading::read_nodes;
use crate::snapshots::models::SnapshotNode;
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

pub(crate) async fn read_snapshot_nodes(
    transaction: ControlTransaction,
    snapshot_id: Uuid,
    selection: Option<&BTreeSet<String>>,
) -> Result<(ControlTransaction, BTreeMap<String, SnapshotNode>), ControlError> {
    if selection.is_some_and(BTreeSet::is_empty) {
        return Ok((transaction, BTreeMap::new()));
    }
    let mut parameters = vec![Parameter::Id(snapshot_id)];
    let filter = selection.map_or(String::new(), |keys| {
        parameters.extend(keys.iter().cloned().map(Parameter::Text));
        format!(
            " AND n.node_key IN ({})",
            (2..parameters.len() + 1)
                .map(|index| format!("${index}"))
                .collect::<Vec<_>>()
                .join(",")
        )
    });
    let (transaction, rows) = query(transaction,
        &format!("SELECT n.node_key, d.body FROM sqb_snapshot_nodes n JOIN sqb_node_definitions d ON d.node_hash=n.node_hash WHERE n.snapshot_id=$1{filter} ORDER BY n.node_key"),
        parameters).await?;
    Ok((transaction, read_nodes(rows)?))
}
