//! Resolve a job's physical writers from its pinned snapshot.

use std::collections::BTreeSet;

use serde_json::Value;
use uuid::Uuid;

use crate::snapshots::constants::{
    LOADER_POINTER, MODEL_NODE_KIND, SOURCE_KEY_PREFIX, SOURCE_NODE_KIND, WRITER_NODE_KINDS,
};
use crate::snapshots::main::read_snapshot_nodes::read_snapshot_nodes;
use crate::store::errors::ControlError;
use crate::store::models::ControlTransaction;

/// Selected writers plus the managed sources a build loads: selected ones and direct parents of selected models.
pub(crate) async fn job_write_set(
    transaction: ControlTransaction,
    snapshot_id: Uuid,
    selection: &BTreeSet<String>,
) -> Result<(ControlTransaction, BTreeSet<String>), ControlError> {
    let (transaction, selected) =
        read_snapshot_nodes(transaction, snapshot_id, Some(selection)).await?;
    let mut writers: BTreeSet<String> = BTreeSet::new();
    let mut sources: BTreeSet<String> = BTreeSet::new();
    for (key, node) in &selected {
        if WRITER_NODE_KINDS.contains(&node.kind.as_str()) {
            writers.insert(key.clone());
        }
        if node.kind == SOURCE_NODE_KIND {
            sources.insert(key.clone());
        } else if node.kind == MODEL_NODE_KIND {
            sources.extend(
                node.definition
                    .get("deps")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .filter(|key| key.starts_with(SOURCE_KEY_PREFIX))
                    .map(str::to_owned),
            );
        }
    }
    let (transaction, sources) =
        read_snapshot_nodes(transaction, snapshot_id, Some(&sources)).await?;
    writers.extend(
        sources
            .into_iter()
            .filter(|(_, node)| {
                node.kind == SOURCE_NODE_KIND
                    && node
                        .definition
                        .pointer(LOADER_POINTER)
                        .is_some_and(|loader| !loader.is_null())
            })
            .map(|(key, _)| key),
    );
    Ok((transaction, writers))
}
