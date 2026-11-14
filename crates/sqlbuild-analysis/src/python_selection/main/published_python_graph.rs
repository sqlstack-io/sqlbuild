use crate::python_selection::_helpers::inventory::{indexes, split_published_deps};
use crate::python_selection::models::{
    InventoryIndexes, InventoryNode, PublishedPythonNode, PythonSelectionGraph, PythonSelectionNode,
};

/// The selection graph of Python nodes rebuilt from a published snapshot, so the server selects them as the CLI does.
pub fn published_python_graph(nodes: &[PublishedPythonNode]) -> PythonSelectionGraph {
    let mut inventory: Vec<InventoryNode> = Vec::with_capacity(nodes.len());
    let mut selection_nodes: Vec<PythonSelectionNode> = Vec::with_capacity(nodes.len());
    for node in nodes {
        let (upstream, sql_deps) = split_published_deps(&node.deps);
        inventory.push(InventoryNode {
            name: node.name.clone(),
            tags: node.tags.clone(),
            relative_path: String::new(),
            upstream,
        });
        selection_nodes.push(PythonSelectionNode {
            name: node.name.clone(),
            kind: node.kind.clone(),
            sql_deps,
        });
    }
    let InventoryIndexes {
        upstream,
        downstream,
        tags,
        ..
    } = indexes(&inventory);
    PythonSelectionGraph {
        nodes: selection_nodes,
        upstream,
        downstream,
        tags,
        paths: nodes
            .iter()
            .map(|node| (node.name.clone(), node.folder.clone()))
            .collect(),
    }
}
