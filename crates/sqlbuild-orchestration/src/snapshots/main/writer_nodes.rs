//! Keep only what each published node contributes to a write set.

use std::collections::BTreeMap;

use crate::snapshots::_helpers::write_set::writer_node;
use crate::snapshots::models::{SnapshotNode, WriterNode};

/// Each node's write-set facts, by key.
pub(crate) fn writer_nodes(nodes: &BTreeMap<String, SnapshotNode>) -> BTreeMap<String, WriterNode> {
    nodes
        .iter()
        .map(|(key, node)| (key.clone(), writer_node(node)))
        .collect()
}
