//! Python's Python-node graph inventory: dependency edges and the tag and path indexes.

use std::collections::HashMap;

use crate::assembly::project::types::ObjectKey;
use crate::definition_changes::constants::PYTHON_NODE_KINDS;

use crate::python_selection::constants::{CURRENT_DIRECTORY, ROOT_DIRECTORY};
use crate::python_selection::models::{InventoryIndexes, InventoryNode};

/// Rows keyed like a Python dict: first insertion keeps its position.
#[derive(Default)]
struct OrderedRows<V> {
    rows: Vec<(String, V)>,
    positions: HashMap<String, usize>,
}

impl<V: Default> OrderedRows<V> {
    fn entry(&mut self, key: &str) -> &mut V {
        let position: usize = match self.positions.get(key) {
            Some(&position) => position,
            None => {
                self.positions.insert(key.to_owned(), self.rows.len());
                self.rows.push((key.to_owned(), V::default()));
                self.rows.len() - 1
            }
        };
        &mut self.rows[position].1
    }
}

/// `PurePosixPath(relative_path).parent.as_posix()` for a normalised POSIX path.
fn parent_folder(relative_path: &str) -> String {
    match relative_path.rsplit_once('/') {
        Some(("", _)) => ROOT_DIRECTORY.to_owned(),
        Some((parent, _)) => parent.to_owned(),
        None => CURRENT_DIRECTORY.to_owned(),
    }
}

fn adjacency(
    nodes: &[InventoryNode],
    edges: &[(String, String)],
    reversed: bool,
) -> Vec<(String, Vec<String>)> {
    let mut rows: OrderedRows<Vec<String>> = OrderedRows::default();
    for node in nodes {
        rows.entry(&node.name);
    }
    for (upstream, downstream) in edges {
        let (from, to) = if reversed {
            (upstream, downstream)
        } else {
            (downstream, upstream)
        };
        rows.entry(from).push(to.clone());
        rows.entry(to);
    }
    rows.rows
}

/// The edges, upstream and downstream adjacency, tag index and path index of the nodes.
pub(crate) fn indexes(nodes: &[InventoryNode]) -> InventoryIndexes {
    let mut edges: Vec<(String, String)> = Vec::new();
    for node in nodes {
        for upstream in &node.upstream {
            edges.push((upstream.clone(), node.name.clone()));
        }
    }
    let mut tags: OrderedRows<Vec<String>> = OrderedRows::default();
    let mut paths: OrderedRows<String> = OrderedRows::default();
    for node in nodes {
        for tag in &node.tags {
            let names: &mut Vec<String> = tags.entry(tag);
            if !names.contains(&node.name) {
                names.push(node.name.clone());
            }
        }
        *paths.entry(&node.name) = parent_folder(&node.relative_path);
    }
    InventoryIndexes {
        upstream: adjacency(nodes, &edges, false),
        downstream: adjacency(nodes, &edges, true),
        edges,
        tags: tags.rows,
        paths: paths.rows,
    }
}

/// A published node's dependency keys split into its Python upstream names and its SQL keys.
pub(crate) fn split_published_deps(deps: &[ObjectKey]) -> (Vec<String>, Vec<ObjectKey>) {
    let mut python: Vec<String> = Vec::new();
    let mut sql: Vec<ObjectKey> = Vec::new();
    for (kind, name) in deps {
        if PYTHON_NODE_KINDS.contains(&kind.as_str()) {
            python.push(name.clone());
        } else {
            sql.push((kind.clone(), name.clone()));
        }
    }
    (python, sql)
}
