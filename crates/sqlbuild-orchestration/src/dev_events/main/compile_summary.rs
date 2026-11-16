//! The compile step's detail on the `sqb dev` screen.

use crate::snapshots::constants::MODEL_NODE_KIND;
use crate::snapshots::models::SnapshotInput;

/// `412 python nodes, 2280 models` for the compiled snapshot.
#[must_use]
pub fn compile_summary(input: &SnapshotInput) -> String {
    let nodes = input.nodes.values();
    let python: usize = nodes
        .clone()
        .filter(|node| node.python_source.is_some())
        .count();
    let models: usize = nodes.filter(|node| node.kind == MODEL_NODE_KIND).count();
    format!("{python} python nodes, {models} models")
}
