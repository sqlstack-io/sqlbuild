use crate::definition_changes::constants::PYTHON_NODE_KINDS;
use crate::python_selection::_helpers::inventory::indexes;
use crate::python_selection::models::{
    InventoryIndexes, InventoryNode, PublishedPythonNode, PythonSelectionGraph, PythonSelectionNode,
};

/// The selection graph of Python nodes rebuilt from a published snapshot, so the server selects them as the CLI does.
pub fn published_python_graph(nodes: &[PublishedPythonNode]) -> PythonSelectionGraph {
    let is_python = |kind: &str| PYTHON_NODE_KINDS.contains(&kind);
    let inventory: Vec<InventoryNode> = nodes
        .iter()
        .map(|node| InventoryNode {
            name: node.name.clone(),
            tags: node.tags.clone(),
            relative_path: String::new(),
            upstream: node
                .deps
                .iter()
                .filter(|(kind, _)| is_python(kind))
                .map(|(_, name)| name.clone())
                .collect(),
        })
        .collect();
    let InventoryIndexes {
        upstream,
        downstream,
        tags,
        ..
    } = indexes(&inventory);
    PythonSelectionGraph {
        nodes: nodes
            .iter()
            .map(|node| PythonSelectionNode {
                name: node.name.clone(),
                kind: node.kind.clone(),
                sql_deps: node
                    .deps
                    .iter()
                    .filter(|(kind, _)| !is_python(kind))
                    .cloned()
                    .collect(),
            })
            .collect(),
        upstream,
        downstream,
        tags,
        paths: nodes
            .iter()
            .map(|node| (node.name.clone(), node.folder.clone()))
            .collect(),
    }
}
