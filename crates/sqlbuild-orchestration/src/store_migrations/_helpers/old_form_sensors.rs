//! Migration 0005's data step: snapshot `triggers` documents lose sensors stored before sensors were functions.

use std::future::Future;
use std::pin::Pin;

use serde_json::Value;
use sqlbuild_recall::records::main::sha256_hex::sha256_hex;

use crate::snapshots::_helpers::content::json_bytes;
use crate::store::errors::ControlError;
use crate::store::main::compress::compress;
use crate::store::main::decompress::decompress;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

/// Rewrite each stored `triggers` document without its old-form sensors, which have neither a `function` nor a `schedule`; their `sqb_triggers` rows stay, hidden by the migration's SQL.
pub(crate) fn drop_old_form_sensors(
    transaction: ControlTransaction,
) -> Pin<Box<dyn Future<Output = Result<ControlTransaction, ControlError>> + Send>> {
    Box::pin(rewrite_documents(transaction))
}

async fn rewrite_documents(
    transaction: ControlTransaction,
) -> Result<ControlTransaction, ControlError> {
    let (mut transaction, rows) = query(
        transaction,
        "SELECT document_hash, body FROM sqb_documents WHERE document_hash IN \
         (SELECT document_hash FROM sqb_snapshot_documents WHERE kind='triggers')",
        Vec::new(),
    )
    .await?;
    for row in rows {
        let invalid = |error: serde_json::Error| ControlError::Invalid(error.to_string());
        let mut document: Value =
            serde_json::from_slice(&decompress(&row["body"])?).map_err(invalid)?;
        let Some(entries) = document.as_object_mut() else {
            continue;
        };
        let before = entries.len();
        entries
            .retain(|_, entry| entry.get("function").is_some() || entry.get("schedule").is_some());
        if entries.len() == before {
            continue;
        }
        let bytes = json_bytes(&document)?;
        let hash = sha256_hex(&bytes);
        (transaction, _) = query(
            transaction,
            "INSERT INTO sqb_documents (document_hash, body) VALUES ($1, $2) ON CONFLICT DO NOTHING",
            vec![Parameter::Text(hash.clone()), Parameter::Bytes(compress(&bytes)?)],
        )
        .await?;
        (transaction, _) = query(
            transaction,
            "UPDATE sqb_snapshot_documents SET document_hash=$2 WHERE kind='triggers' AND document_hash=$1",
            vec![Parameter::Text(row["document_hash"].as_str().unwrap_or_default().to_owned()), Parameter::Text(hash)],
        )
        .await?;
    }
    Ok(transaction)
}
