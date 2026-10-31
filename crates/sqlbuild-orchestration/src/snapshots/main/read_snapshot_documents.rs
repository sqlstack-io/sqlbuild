//! Decode immutable documents within the caller's consistent snapshot transaction.

use std::collections::BTreeMap;

use serde_json::Value;
use uuid::Uuid;

use crate::snapshots::_helpers::reading::read_documents;
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

pub(crate) async fn read_snapshot_documents(
    transaction: ControlTransaction,
    snapshot_id: Uuid,
    kinds: Option<&[&str]>,
) -> Result<(ControlTransaction, BTreeMap<String, Value>), ControlError> {
    if kinds.is_some_and(<[&str]>::is_empty) {
        return Ok((transaction, BTreeMap::new()));
    }
    let mut parameters = vec![Parameter::Id(snapshot_id)];
    let filter = kinds.map_or(String::new(), |keys| {
        parameters.extend(keys.iter().map(|key| Parameter::Text((*key).into())));
        format!(
            " AND d.kind IN ({})",
            (2..parameters.len() + 1)
                .map(|index| format!("${index}"))
                .collect::<Vec<_>>()
                .join(",")
        )
    });
    let (transaction, rows) = query(transaction,
        &format!("SELECT d.kind, b.body FROM sqb_snapshot_documents d JOIN sqb_documents b ON b.document_hash=d.document_hash WHERE d.snapshot_id=$1{filter}"),
        parameters).await?;
    Ok((transaction, read_documents(rows)?))
}
