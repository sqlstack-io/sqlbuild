//! The selected nodes that read a provider's users, directly or through other selected nodes.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use sqlbuild_analysis::graph::models::ProjectGraph;
use sqlbuild_python_project::graph::models::ProjectSelection;

/// `kind:name` of an object key.
fn key_text((kind, name): &(String, String)) -> String {
    format!("{kind}:{name}")
}

/// Each key's direct readers: SQL lineage edges and Python nodes' upstream keys, inverted.
pub(crate) fn readers_by_key(
    graph: &ProjectGraph,
    selection: &ProjectSelection,
) -> BTreeMap<String, BTreeSet<String>> {
    let mut readers: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (key, downstream) in &graph.downstream {
        let read_by: BTreeSet<String> = downstream.iter().map(key_text).collect();
        readers.entry(key_text(key)).or_default().extend(read_by);
    }
    for (node, deps) in selection.node_deps() {
        for dep in deps {
            readers.entry(dep).or_default().insert(node.clone());
        }
    }
    readers
}

/// Selected keys downstream of `users`, walking only through selected nodes.
pub(crate) fn downstream_readers(
    users: &BTreeSet<String>,
    selected: &BTreeSet<String>,
    readers: &BTreeMap<String, BTreeSet<String>>,
) -> Vec<String> {
    let mut found: BTreeSet<String> = BTreeSet::new();
    let mut pending: VecDeque<String> = users.iter().cloned().collect();
    while let Some(key) = pending.pop_front() {
        for reader in readers.get(&key).into_iter().flatten() {
            if selected.contains(reader) && !users.contains(reader) && found.insert(reader.clone())
            {
                pending.push_back(reader.clone());
            }
        }
    }
    found.into_iter().collect()
}
