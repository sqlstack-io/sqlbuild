//! Only the supported Recall views can be read; no caller-provided SQL is evaluated.

use sqlbuild_recall::schema::constants::VIEW_NAMES;

use crate::store::errors::ControlError;
use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::main::query::query;
use crate::store::models::{ControlStore, Parameter, TransactionMode};
use crate::store::types::Row;

/// Read authorized evidence, decoding only display text while preserving byte-exact captured content.
pub async fn read_recall_view(
    store: &ControlStore,
    (view, invocation_id): (&str, &str),
    limit: u32,
) -> Result<Vec<Row>, ControlError> {
    if !VIEW_NAMES.contains(&view) {
        return Err(ControlError::Invalid("unknown Recall view".into()));
    }
    let transaction = begin(store, TransactionMode::Read).await?;
    let (transaction, mut rows) = query(
        transaction,
        &format!("SELECT * FROM \"{view}\" WHERE invocation_id=$1 LIMIT $2"),
        vec![
            Parameter::Text(invocation_id.into()),
            Parameter::Integer(i64::from(limit.min(1000))),
        ],
    )
    .await?;
    if matches!(view, "output_tail" | "artifacts") {
        for row in &mut rows {
            if let Some(text) = row.get_mut("text")
                && text.is_array()
            {
                let bytes: Vec<u8> = serde_json::from_value(text.clone())
                    .map_err(|error| ControlError::Invalid(error.to_string()))?;
                *text = serde_json::Value::String(String::from_utf8_lossy(&bytes).into_owned());
            }
        }
    }
    commit(transaction).await?;
    Ok(rows)
}
