//! Resolve a job's physical writers from its pinned snapshot.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;
use uuid::Uuid;

use crate::snapshots::constants::{
    LOAD_SOURCES_SETTING, LOADER_POINTER, LOADS_FIELD, MODEL_NODE_KIND, SOURCE_KEY_PREFIX,
    SOURCE_NODE_KIND, WRITER_NODE_KINDS,
};
use crate::snapshots::main::read_snapshot_nodes::read_snapshot_nodes;
use crate::store::errors::ControlError;
use crate::store::models::ControlTransaction;

/// Selected writers plus the managed sources a build loads: selected ones, and unless loading is off, those
/// selected models read or list in `loads` with each one's `loads` loader chain. The worker runs a selected
/// source's own loader only, so a source no selected model reads claims no chain.
pub(crate) async fn job_write_set(
    transaction: ControlTransaction,
    snapshot_id: Uuid,
    (selection, settings): (&BTreeSet<String>, &BTreeMap<String, Value>),
) -> Result<(ControlTransaction, BTreeSet<String>), ControlError> {
    let (transaction, selected) =
        read_snapshot_nodes(transaction, snapshot_id, Some(selection)).await?;
    let loads_model_sources = settings.get(LOAD_SOURCES_SETTING) != Some(&Value::Bool(false));
    let mut writers: BTreeSet<String> = BTreeSet::new();
    let mut sources: BTreeSet<String> = BTreeSet::new();
    let mut model_reads: BTreeSet<String> = BTreeSet::new();
    for (key, node) in &selected {
        if WRITER_NODE_KINDS.contains(&node.kind.as_str()) {
            writers.insert(key.clone());
        }
        if node.kind == SOURCE_NODE_KIND {
            sources.insert(key.clone());
        } else if node.kind == MODEL_NODE_KIND && loads_model_sources {
            model_reads.extend(
                node.definition
                    .get("deps")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .filter(|key| key.starts_with(SOURCE_KEY_PREFIX))
                    .map(str::to_owned),
            );
            model_reads.extend(loads(&node.definition));
        }
    }
    sources.extend(model_reads.iter().cloned());
    let (transaction, sources) =
        read_snapshot_nodes(transaction, snapshot_id, Some(&sources)).await?;
    for (key, node) in sources {
        if node.kind == SOURCE_NODE_KIND
            && node
                .definition
                .pointer(LOADER_POINTER)
                .is_some_and(|loader| !loader.is_null())
        {
            if model_reads.contains(&key) {
                writers.extend(loads(&node.definition));
            }
            writers.insert(key);
        }
    }
    Ok((transaction, writers))
}

/// The keys a node's definition lists in `loads`; snapshots published before it have none.
fn loads(definition: &Value) -> impl Iterator<Item = String> + '_ {
    definition
        .get(LOADS_FIELD)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_owned)
}
