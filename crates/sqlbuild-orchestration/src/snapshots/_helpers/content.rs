//! Canonical content addresses and compressed immutable bodies.

use std::collections::BTreeMap;
use std::io::{Read, Write};

use flate2::{Compression, read::GzDecoder, write::GzEncoder};
use serde_json::Value;
use sqlbuild_recall::records::main::sha256_hex::sha256_hex;

use crate::snapshots::models::{PreparedNode, SnapshotNode};
use crate::store::errors::ControlError;

fn canonical(value: &Value) -> Value {
    match value {
        Value::Object(values) => {
            let sorted: BTreeMap<_, _> = values
                .iter()
                .map(|(key, value)| (key.clone(), canonical(value)))
                .collect();
            Value::Object(sorted.into_iter().collect())
        }
        Value::Array(values) => Value::Array(values.iter().map(canonical).collect()),
        _ => value.clone(),
    }
}

pub(crate) fn json_bytes(value: &Value) -> Result<Vec<u8>, ControlError> {
    serde_json::to_vec(&canonical(value)).map_err(|error| ControlError::Invalid(error.to_string()))
}

pub(crate) fn compress(bytes: &[u8]) -> Result<Vec<u8>, ControlError> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(bytes)?;
    Ok(encoder.finish()?)
}

pub(crate) fn decompress(value: &Value) -> Result<Vec<u8>, ControlError> {
    let bytes: Vec<u8> = serde_json::from_value(value.clone())
        .map_err(|error| ControlError::Invalid(error.to_string()))?;
    let mut output: Vec<u8> = Vec::new();
    GzDecoder::new(bytes.as_slice()).read_to_end(&mut output)?;
    Ok(output)
}

pub(crate) fn prepare_node(node: &SnapshotNode) -> Result<PreparedNode, ControlError> {
    let mut sources: BTreeMap<&'static str, (String, Vec<u8>)> = BTreeMap::new();
    for (kind, text) in [
        ("raw_sql", &node.raw_sql),
        ("compiled_sql", &node.compiled_sql),
        ("python_source", &node.python_source),
    ] {
        if let Some(text) = text {
            sources.insert(
                kind,
                (sha256_hex(text.as_bytes()), compress(text.as_bytes())?),
            );
        }
    }
    let source_hashes: BTreeMap<_, _> = sources
        .iter()
        .map(|(kind, (hash, _))| (*kind, hash))
        .collect();
    let definition = json_bytes(
        &serde_json::json!({"kind": node.kind, "definition": node.definition, "sources": source_hashes}),
    )?;
    Ok(PreparedNode {
        kind: node.kind.clone(),
        hash: sha256_hex(&definition),
        definition: compress(&definition)?,
        sources,
    })
}
