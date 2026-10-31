//! Reconcile publication identity without reading node definitions, source bodies or documents.

use uuid::Uuid;

use crate::snapshots::_helpers::reading::read_current_record;
use crate::snapshots::models::SnapshotRecord;
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit};
use crate::store::models::{ControlStore, TransactionMode};

pub async fn current_snapshot_record(
    store: &ControlStore,
    location_id: Uuid,
    target: &str,
) -> Result<Option<SnapshotRecord>, ControlError> {
    let (transaction, record) = read_current_record(
        begin(store, TransactionMode::Read).await?,
        (location_id, target),
    )
    .await?;
    commit(transaction).await?;
    Ok(record)
}
