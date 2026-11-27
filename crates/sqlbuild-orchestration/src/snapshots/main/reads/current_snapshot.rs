//! Read the current generation without locking out snapshot publishers; its nodes are read in short transactions within the memory budget.

use uuid::Uuid;

use crate::snapshots::_helpers::reading::attach_sources;
use crate::snapshots::_helpers::whole_snapshot::{SNAPSHOT_NODES, read_budgeted};
use crate::snapshots::main::reads::read_current_record::read_current_record;
use crate::snapshots::main::reads::read_snapshot_documents::read_snapshot_documents;
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
    commit(transaction).await?;
    let Some(record) = record else {
        return Ok(None);
    };
    let nodes = read_budgeted(&SNAPSHOT_NODES, store, record.snapshot_id).await?;
    let transaction = begin(store, TransactionMode::Read).await?;
    let select = format!(
        "SELECT s.node_key, s.kind, {} AS body FROM sqb_snapshot_sources s JOIN sqb_source_texts t ON t.text_hash=s.text_hash WHERE s.snapshot_id=$1",
        transaction.hex_column("t.body"),
    );
    let (transaction, rows) = query(
        transaction,
        &select,
        vec![Parameter::Id(record.snapshot_id)],
    )
    .await?;
    let nodes = tokio::task::spawn_blocking(move || attach_sources(nodes, rows)).await??;
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
