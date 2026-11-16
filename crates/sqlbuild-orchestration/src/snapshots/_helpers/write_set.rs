//! Write-set pieces read from published node definitions.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use crate::snapshots::constants::{
    LOAD_SOURCES_SETTING, LOADER_POINTER, LOADS_FIELD, MODEL_NODE_KIND, SOURCE_KEY_PREFIX,
    SOURCE_NODE_KIND, WRITER_NODE_KINDS,
};
use crate::snapshots::models::{SnapshotNode, WriterNode};

/// The parts of a published node a write set reads.
pub(crate) fn writer_node(node: &SnapshotNode) -> WriterNode {
    let definition = &node.definition;
    WriterNode {
        kind: node.kind.clone(),
        source_deps: keys(definition, "deps")
            .filter(|key| key.starts_with(SOURCE_KEY_PREFIX))
            .collect(),
        loads: keys(definition, LOADS_FIELD).collect(),
        managed: node.kind == SOURCE_NODE_KIND
            && definition
                .pointer(LOADER_POINTER)
                .is_some_and(|loader| !loader.is_null()),
    }
}

/// Whether a build with these settings loads the managed sources its selected models read.
pub(crate) fn loads_model_sources(settings: &BTreeMap<String, Value>) -> bool {
    settings.get(LOAD_SOURCES_SETTING) != Some(&Value::Bool(false))
}

/// Selected writer keys, candidate source keys, and the sources selected models read when loading is on.
pub(crate) fn selected_writers<'a>(
    selected: impl IntoIterator<Item = (&'a String, &'a WriterNode)>,
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
            model_reads.extend(node.source_deps.iter().cloned());
            model_reads.extend(node.loads.iter().cloned());
        }
    }
    sources.extend(model_reads.iter().cloned());
    (writers, sources, model_reads)
}

/// The managed candidate sources, and the loader chains of those selected models read.
pub(crate) fn managed_sources<'a>(
    candidates: impl IntoIterator<Item = (&'a String, &'a WriterNode)>,
    model_reads: &BTreeSet<String>,
) -> BTreeSet<String> {
    let mut written: BTreeSet<String> = BTreeSet::new();
    for (key, node) in candidates.into_iter().filter(|(_, node)| node.managed) {
        if model_reads.contains(key) {
            written.extend(node.loads.iter().cloned());
        }
        written.insert(key.clone());
    }
    written
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
