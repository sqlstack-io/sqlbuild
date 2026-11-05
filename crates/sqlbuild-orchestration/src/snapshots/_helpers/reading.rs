//! Decode stored bodies and attach source text without evaluating it.

use std::collections::BTreeMap;

use crate::snapshots::models::SnapshotNode;
use crate::store::errors::ControlError;
use crate::store::main::decompress::decompress;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};
use crate::store::types::Row;
use serde::de::DeserializeOwned;
use serde_json::Value;
use uuid::Uuid;

fn decode_body<T: DeserializeOwned>(value: &Value) -> Result<T, ControlError> {
    serde_json::from_slice(&decompress(value)?)
        .map_err(|error| ControlError::Invalid(error.to_string()))
}

/// One snapshot's keyed bodies, all of them or only the listed keys; an empty list reads none.
pub(crate) async fn query_bodies<K: ToString>(
    transaction: ControlTransaction,
    (select, key_column): (&str, &str),
    snapshot_id: Uuid,
    keys: Option<impl IntoIterator<Item = K>>,
) -> Result<(ControlTransaction, Vec<Row>), ControlError> {
    let mut parameters = vec![Parameter::Id(snapshot_id)];
    let mut filter = String::new();
    if let Some(keys) = keys {
        parameters.extend(keys.into_iter().map(|key| Parameter::Text(key.to_string())));
        if parameters.len() == 1 {
            return Ok((transaction, Vec::new()));
        }
        filter = format!(
            " AND {key_column} IN ({})",
            Parameter::placeholders(2..=parameters.len())
        );
    }
    query(transaction, &format!("{select}{filter}"), parameters).await
}

pub(crate) fn read_nodes(rows: Vec<Row>) -> Result<BTreeMap<String, SnapshotNode>, ControlError> {
    read_bodies(rows, "node_key")
}

fn read_bodies<T: DeserializeOwned>(
    rows: Vec<Row>,
    key: &str,
) -> Result<BTreeMap<String, T>, ControlError> {
    rows.into_iter()
        .map(|row| {
            let body: T = decode_body(&row["body"])?;
            Ok((row[key].as_str().unwrap_or_default().to_owned(), body))
        })
        .collect()
}

pub(crate) fn attach_sources(
    mut nodes: BTreeMap<String, SnapshotNode>,
    rows: Vec<Row>,
) -> Result<BTreeMap<String, SnapshotNode>, ControlError> {
    for row in rows {
        let key = row["node_key"].as_str().unwrap_or_default();
        let node = nodes
            .get_mut(key)
            .ok_or_else(|| ControlError::Invalid("snapshot source has no node".into()))?;
        let text = String::from_utf8(decompress(&row["body"])?)
            .map_err(|error| ControlError::Invalid(error.to_string()))?;
        match row["kind"].as_str() {
            Some("raw_sql") => node.raw_sql = Some(text),
            Some("compiled_sql") => node.compiled_sql = Some(text),
            Some("python_source") => node.python_source = Some(text),
            _ => return Err(ControlError::Invalid("unknown stored source kind".into())),
        }
    }
    Ok(nodes)
}

pub(crate) fn read_documents(rows: Vec<Row>) -> Result<BTreeMap<String, Value>, ControlError> {
    read_bodies(rows, "kind")
}
