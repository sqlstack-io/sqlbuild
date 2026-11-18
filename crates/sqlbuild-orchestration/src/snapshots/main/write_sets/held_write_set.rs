//! Resolve a selection's physical writers from write-set facts already held in memory.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use crate::snapshots::_helpers::write_set::{
    loads_model_sources, managed_sources, selected_writers,
};
use crate::snapshots::models::WriterNode;

/// [`crate::snapshots::main::write_sets::job_write_set::job_write_set`] over every node's facts, read once.
pub(crate) fn held_write_set(
    nodes: &BTreeMap<String, WriterNode>,
    (selection, settings): (&BTreeSet<String>, &BTreeMap<String, Value>),
) -> BTreeSet<String> {
    let present = |keys: &BTreeSet<String>| -> Vec<(&String, &WriterNode)> {
        keys.iter()
            .filter_map(|key| nodes.get_key_value(key))
            .collect()
    };
    let (mut written, candidates, model_reads) =
        selected_writers(present(selection), loads_model_sources(settings));
    written.extend(managed_sources(present(&candidates), &model_reads));
    written
}
