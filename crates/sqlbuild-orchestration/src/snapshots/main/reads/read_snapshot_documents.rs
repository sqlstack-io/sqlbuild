//! Decode immutable documents within the caller's consistent snapshot transaction.

use std::collections::BTreeMap;

use serde_json::Value;
use uuid::Uuid;

use crate::snapshots::_helpers::reading::{query_bodies, read_documents};
use crate::store::errors::ControlError;
use crate::store::models::ControlTransaction;

pub(crate) async fn read_snapshot_documents(
    transaction: ControlTransaction,
    snapshot_id: Uuid,
    kinds: Option<&[&str]>,
) -> Result<(ControlTransaction, BTreeMap<String, Value>), ControlError> {
    let (transaction, rows) = query_bodies(
        transaction,
        (
            "SELECT d.kind, b.body FROM sqb_snapshot_documents d JOIN sqb_documents b \
             ON b.document_hash=d.document_hash WHERE d.snapshot_id=$1",
            "d.kind",
        ),
        snapshot_id,
        kinds,
    )
    .await?;
    Ok((transaction, read_documents(rows)?))
}
