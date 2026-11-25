//! Migration 0012's data step: sensors watching `partial` runs watch `failed` ones, in `triggers` documents and trigger rows.

use std::collections::BTreeSet;
use std::future::Future;
use std::pin::Pin;

use serde_json::Value;

use crate::sensors::models::RunEventKind;
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};
use crate::store_migrations::_helpers::trigger_documents::rewrite_trigger_documents;

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
    rewrite_trigger_documents(transaction, |entries| {
        let mut changed = false;
        for entry in entries.values_mut() {
            changed |= watch_failed(entry)?;
        }
        Ok(changed)
    })
    .await
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
