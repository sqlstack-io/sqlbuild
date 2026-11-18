//! Resolve a job's physical writers from its pinned snapshot.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;
use uuid::Uuid;

use crate::snapshots::_helpers::write_set::{
    loads_model_sources, managed_sources, selected_writers,
};
use crate::snapshots::main::reads::read_snapshot_nodes::read_snapshot_nodes;
use crate::snapshots::main::write_sets::writer_nodes::writer_nodes;
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
    let (mut written, candidates, model_reads) =
        selected_writers(&writer_nodes(&selected), loads_model_sources(settings));
    let (transaction, sources) =
        read_snapshot_nodes(transaction, snapshot_id, Some(&candidates)).await?;
    written.extend(managed_sources(&writer_nodes(&sources), &model_reads));
    Ok((transaction, written))
}
