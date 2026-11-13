//! Resolve a job's physical writers from its pinned snapshot.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;
use uuid::Uuid;

use crate::snapshots::_helpers::write_set::{loads, managed, selected_writers};
use crate::snapshots::constants::LOAD_SOURCES_SETTING;
use crate::snapshots::main::read_snapshot_nodes::read_snapshot_nodes;
use crate::store::errors::ControlError;
use crate::store::models::ControlTransaction;

/// Selected writers, the managed sources a build loads, and loader chains of sources selected models read; a selected source runs only its own loader.
pub(crate) async fn job_write_set(
    transaction: ControlTransaction,
    snapshot_id: Uuid,
    (selection, settings): (&BTreeSet<String>, &BTreeMap<String, Value>),
) -> Result<(ControlTransaction, BTreeSet<String>), ControlError> {
    let (transaction, selected) =
        read_snapshot_nodes(transaction, snapshot_id, Some(selection)).await?;
    let loads_model_sources = settings.get(LOAD_SOURCES_SETTING) != Some(&Value::Bool(false));
    let (mut writers, candidates, model_reads) = selected_writers(&selected, loads_model_sources);
    let (transaction, sources) =
        read_snapshot_nodes(transaction, snapshot_id, Some(&candidates)).await?;
    for (key, node) in sources.into_iter().filter(|(_, node)| managed(node)) {
        if model_reads.contains(&key) {
            writers.extend(loads(&node.definition));
        }
        writers.insert(key);
    }
    Ok((transaction, writers))
}
