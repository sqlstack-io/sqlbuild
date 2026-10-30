//! Read one consistent generation without locking out snapshot publishers.

use uuid::Uuid;

use crate::snapshots::_helpers::reading::{attach_sources, read_documents, read_nodes};
use crate::snapshots::models::{CurrentSnapshot, SnapshotChange, SnapshotRecord};
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
    let transaction = begin(store, TransactionMode::Read).await?;
    let (transaction, rows) = query(transaction,
        "SELECT s.snapshot_id, s.location_id, s.target, s.generation, s.manifest_digest, s.published_at \
         FROM sqb_current_snapshots c JOIN sqb_snapshots s ON s.snapshot_id=c.snapshot_id WHERE c.location_id=$1 AND c.target=$2",
        vec![Parameter::Id(location_id), Parameter::Text(target.into())]).await?;
    let Some(row) = rows.into_iter().next() else {
        return Ok(None);
    };
    let record: SnapshotRecord = decode_row(row)?;
    let (transaction, rows) = query(transaction,
        "SELECT n.node_key, d.body FROM sqb_snapshot_nodes n JOIN sqb_node_definitions d ON d.node_hash=n.node_hash WHERE n.snapshot_id=$1 ORDER BY n.node_key",
        vec![Parameter::Id(record.snapshot_id)]).await?;
    let nodes = read_nodes(rows)?;
    let (transaction, rows) = query(transaction,
        "SELECT s.node_key, s.kind, t.body FROM sqb_snapshot_sources s JOIN sqb_source_texts t ON t.text_hash=s.text_hash WHERE s.snapshot_id=$1",
        vec![Parameter::Id(record.snapshot_id)]).await?;
    let nodes = attach_sources(nodes, rows)?;
    let (transaction, rows) = query(transaction,
        "SELECT d.kind, b.body FROM sqb_snapshot_documents d JOIN sqb_documents b ON b.document_hash=d.document_hash WHERE d.snapshot_id=$1",
        vec![Parameter::Id(record.snapshot_id)]).await?;
    let documents = read_documents(rows)?;
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
