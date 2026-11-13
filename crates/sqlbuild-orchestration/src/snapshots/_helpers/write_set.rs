//! Write-set pieces read from published node definitions.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use crate::snapshots::constants::{
    LOADER_POINTER, LOADS_FIELD, MODEL_NODE_KIND, SOURCE_KEY_PREFIX, SOURCE_NODE_KIND,
    WRITER_NODE_KINDS,
};
use crate::snapshots::models::SnapshotNode;

/// Selected writer keys, candidate source keys, and the sources selected models read when loading is on.
pub(crate) fn selected_writers(
    selected: &BTreeMap<String, SnapshotNode>,
    loads_model_sources: bool,
) -> (BTreeSet<String>, BTreeSet<String>, BTreeSet<String>) {
    let mut writers: BTreeSet<String> = BTreeSet::new();
    let mut sources: BTreeSet<String> = BTreeSet::new();
    let mut model_reads: BTreeSet<String> = BTreeSet::new();
    for (key, node) in selected {
        if WRITER_NODE_KINDS.contains(&node.kind.as_str()) {
            writers.insert(key.clone());
        }
        if node.kind == SOURCE_NODE_KIND {
            sources.insert(key.clone());
        } else if node.kind == MODEL_NODE_KIND && loads_model_sources {
            model_reads.extend(source_deps(&node.definition));
            model_reads.extend(loads(&node.definition));
        }
    }
    sources.extend(model_reads.iter().cloned());
    (writers, sources, model_reads)
}

/// A source with a non-null loader, which a loading build writes.
pub(crate) fn managed(node: &SnapshotNode) -> bool {
    node.kind == SOURCE_NODE_KIND
        && node
            .definition
            .pointer(LOADER_POINTER)
            .is_some_and(|loader| !loader.is_null())
}

/// The source keys among a node's compiled `deps`.
pub(crate) fn source_deps(definition: &Value) -> impl Iterator<Item = String> + '_ {
    keys(definition, "deps").filter(|key| key.starts_with(SOURCE_KEY_PREFIX))
}

/// The keys a node's definition lists in `loads`; snapshots published before it have none.
pub(crate) fn loads(definition: &Value) -> impl Iterator<Item = String> + '_ {
    keys(definition, LOADS_FIELD)
}

/// The string keys a node's definition lists under `field`.
fn keys<'a>(definition: &'a Value, field: &str) -> impl Iterator<Item = String> + 'a {
    definition
        .get(field)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_owned)
}
