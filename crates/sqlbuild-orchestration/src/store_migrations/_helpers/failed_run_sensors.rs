//! Migration 0012's data step: sensors watching `partial` runs watch `failed` ones, in `triggers` documents and trigger rows.

use std::collections::BTreeSet;
use std::future::Future;
use std::pin::Pin;

use serde_json::Value;

use crate::sensors::models::RunEventKind;
use crate::snapshots::main::publication::stored_document::stored_document;
use crate::store::errors::ControlError;
use crate::store::main::decompress::decompress;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

/// The run outcome 0012 folds into `failed`.
const PARTIAL: &str = "partial";

/// Rewrite each stored `triggers` document and sensor row whose `run_event` condition watches `partial` to watch `failed`.
pub(crate) fn watch_failed_runs(
    transaction: ControlTransaction,
) -> Pin<Box<dyn Future<Output = Result<ControlTransaction, ControlError>> + Send>> {
    Box::pin(rewrite_sensors(transaction))
}

async fn rewrite_sensors(
    transaction: ControlTransaction,
) -> Result<ControlTransaction, ControlError> {
    let transaction = rewrite_documents(transaction).await?;
    let (mut transaction, rows) = query(
        transaction,
        "SELECT trigger_id, definition_json FROM sqb_triggers WHERE kind='sensor'",
        Vec::new(),
    )
    .await?;
    for row in rows {
        let mut definition = json_value(&row["definition_json"])?;
        if !watch_failed(&mut definition)? {
            continue;
        }
        let trigger_id = uuid::Uuid::parse_str(row["trigger_id"].as_str().unwrap_or_default())
            .map_err(|error| ControlError::Schema(format!("stored trigger ID: {error}")))?;
        (transaction, _) = query(
            transaction,
            "UPDATE sqb_triggers SET definition_json=$2 WHERE trigger_id=$1",
            vec![Parameter::Id(trigger_id), Parameter::Json(definition)],
        )
        .await?;
    }
    Ok(transaction)
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
        let mut document: Value = serde_json::from_slice(&decompress(&row["body"])?)
            .map_err(|error| ControlError::Invalid(error.to_string()))?;
        let Some(entries) = document.as_object_mut() else {
            continue;
        };
        let mut changed = false;
        for entry in entries.values_mut() {
            changed |= watch_failed(entry)?;
        }
        if !changed {
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

/// A JSON column as read back: a value, or its text on a store that returns JSON as text.
fn json_value(column: &Value) -> Result<Value, ControlError> {
    match column.as_str() {
        Some(text) => {
            serde_json::from_str(text).map_err(|error| ControlError::Invalid(error.to_string()))
        }
        None => Ok(column.clone()),
    }
}

/// Whether `sensor`'s `run_event` condition watched `partial`; it then watches `failed` in its place.
fn watch_failed(sensor: &mut Value) -> Result<bool, ControlError> {
    let watches_partial = sensor.pointer("/when/kind").and_then(Value::as_str) == Some("run_event")
        && sensor
            .pointer("/when/kinds")
            .and_then(Value::as_array)
            .is_some_and(|kinds| kinds.iter().any(|kind| kind.as_str() == Some(PARTIAL)));
    let Some(kinds) = sensor
        .pointer_mut("/when/kinds")
        .filter(|_| watches_partial)
    else {
        return Ok(false);
    };
    let watched: BTreeSet<RunEventKind> = kinds
        .as_array()
        .into_iter()
        .flatten()
        .filter(|kind| kind.as_str() != Some(PARTIAL))
        .map(|kind| serde_json::from_value(kind.clone()))
        .chain([Ok(RunEventKind::Failed)])
        .collect::<Result<_, _>>()
        .map_err(|error| ControlError::Invalid(error.to_string()))?;
    *kinds =
        serde_json::to_value(watched).map_err(|error| ControlError::Invalid(error.to_string()))?;
    Ok(true)
}
