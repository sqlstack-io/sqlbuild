//! Read one consistent generation without locking out snapshot publishers.

use uuid::Uuid;

use crate::snapshots::_helpers::reading::{attach_sources, read_current_record};
use crate::snapshots::main::read_snapshot_documents::read_snapshot_documents;
use crate::snapshots::main::read_snapshot_nodes::read_snapshot_nodes;
use crate::snapshots::models::{CurrentSnapshot, SnapshotChange};
use crate::store::errors::ControlError;
use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::main::decode_row::decode_row;
use crate::store::main::query::query;
use crate::store::models::{ControlStore, Parameter, TransactionMode};

/// The last successfully published generation, or none before the first successful compile.
pub async fn current_snapshot(
    store: &ControlStore,
    location_id: Uuid,
    target: &str,
) -> Result<Option<CurrentSnapshot>, ControlError> {
    let (transaction, record) = read_current_record(
        begin(store, TransactionMode::Read).await?,
        (location_id, target),
    )
    .await?;
    let Some(record) = record else {
        return Ok(None);
    };
    let (transaction, nodes) = read_snapshot_nodes(transaction, record.snapshot_id, None).await?;
    let (transaction, rows) = query(transaction,
        "SELECT s.node_key, s.kind, t.body FROM sqb_snapshot_sources s JOIN sqb_source_texts t ON t.text_hash=s.text_hash WHERE s.snapshot_id=$1",
        vec![Parameter::Id(record.snapshot_id)]).await?;
    let nodes = attach_sources(nodes, rows)?;
    let (transaction, documents) =
        read_snapshot_documents(transaction, record.snapshot_id, None).await?;
    let (transaction, rows) = query(transaction,
        "SELECT change_kind, subject_key, before_hash, after_hash FROM sqb_snapshot_changes WHERE snapshot_id=$1 ORDER BY change_seq",
        vec![Parameter::Id(record.snapshot_id)]).await?;
    let changes: Vec<SnapshotChange> =
        rows.into_iter().map(decode_row).collect::<Result<_, _>>()?;
    commit(transaction).await?;
    Ok(Some(CurrentSnapshot {
        record,
        nodes,
        documents,
        changes,
    }))
}
