//! Rewrite the stored `triggers` documents a migration's data step changes, keeping the others as they are.

use serde_json::{Map, Value};

use crate::snapshots::main::publication::stored_document::stored_document;
use crate::store::errors::ControlError;
use crate::store::main::decompress::decompress;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

/// Store each `triggers` document that `edit` changes, by trigger name, under its new hash, and point the snapshots holding it there.
pub(crate) async fn rewrite_trigger_documents(
    transaction: ControlTransaction,
    edit: impl Fn(&mut Map<String, Value>) -> Result<bool, ControlError> + Send,
) -> Result<ControlTransaction, ControlError> {
    let select = format!(
        "SELECT document_hash, {} AS body FROM sqb_documents WHERE document_hash IN \
         (SELECT document_hash FROM sqb_snapshot_documents WHERE kind='triggers')",
        transaction.hex_column("body"),
    );
    let (mut transaction, rows) = query(transaction, &select, Vec::new()).await?;
    for row in rows {
        let mut document: Value = serde_json::from_slice(&decompress(&row["body"])?)
            .map_err(|error| ControlError::Invalid(error.to_string()))?;
        let Some(entries) = document.as_object_mut() else {
            continue;
        };
        if !edit(entries)? {
            continue;
        }
        let (hash, body) = stored_document(&document)?;
        (transaction, _) = query(
            transaction,
            "INSERT INTO sqb_documents (document_hash, body) VALUES ($1, $2) ON CONFLICT DO NOTHING",
            vec![Parameter::Text(hash.clone()), Parameter::Bytes(body)],
        )
        .await?;
        (transaction, _) = query(
            transaction,
            "UPDATE sqb_snapshot_documents SET document_hash=$2 WHERE kind='triggers' AND document_hash=$1",
            vec![
                Parameter::Text(row["document_hash"].as_str().unwrap_or_default().to_owned()),
                Parameter::Text(hash),
            ],
        )
        .await?;
    }
    Ok(transaction)
}
