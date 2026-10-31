//! Decode stored bodies and attach source text without evaluating it.

use std::collections::BTreeMap;

use serde::de::DeserializeOwned;
use serde_json::Value;
use uuid::Uuid;

use crate::snapshots::_helpers::content::decompress;
use crate::snapshots::models::{SnapshotNode, SnapshotRecord};
use crate::store::errors::ControlError;
use crate::store::main::{decode_row::decode_row, query::query};
use crate::store::models::{ControlTransaction, Parameter};
use crate::store::types::Row;

pub(crate) async fn read_current_record(
    transaction: ControlTransaction,
    (location_id, target): (Uuid, &str),
) -> Result<(ControlTransaction, Option<SnapshotRecord>), ControlError> {
    let (transaction, rows) = query(transaction,
        "SELECT s.snapshot_id, s.location_id, s.target, s.generation, s.manifest_digest, s.published_at \
         FROM sqb_current_snapshots c JOIN sqb_snapshots s ON s.snapshot_id=c.snapshot_id WHERE c.location_id=$1 AND c.target=$2",
        vec![Parameter::Id(location_id), Parameter::Text(target.into())]).await?;
    Ok((
        transaction,
        rows.into_iter().next().map(decode_row).transpose()?,
    ))
}

fn decode_body<T: DeserializeOwned>(value: &Value) -> Result<T, ControlError> {
    serde_json::from_slice(&decompress(value)?)
        .map_err(|error| ControlError::Invalid(error.to_string()))
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
