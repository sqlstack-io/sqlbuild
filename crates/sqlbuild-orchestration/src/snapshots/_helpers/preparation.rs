//! Validate and content-address a successful compile before holding a database write lock.

use std::collections::BTreeMap;

use sqlbuild_recall::records::main::sha256_hex::sha256_hex;

use crate::snapshots::_helpers::content::{json_bytes, prepare_node};
use crate::snapshots::models::{PreparedNode, PreparedSnapshot, SnapshotInput};
use crate::store::errors::ControlError;
use crate::store::main::compress::compress;

pub(crate) fn prepare_snapshot(input: &SnapshotInput) -> Result<PreparedSnapshot, ControlError> {
    if input.target.trim().is_empty()
        || input.environment_hash.is_empty()
        || input.generation <= 0
        || input
            .nodes
            .iter()
            .any(|(key, node)| key.is_empty() || node.kind.is_empty())
    {
        return Err(ControlError::Invalid(
            "target, environment, positive generation, and named nodes are required".into(),
        ));
    }
    let mut nodes: BTreeMap<String, PreparedNode> = BTreeMap::new();
    for (key, node) in &input.nodes {
        nodes.insert(key.clone(), prepare_node(node)?);
    }
    let hashes: BTreeMap<String, String> = nodes
        .iter()
        .map(|(key, node)| (key.clone(), node.hash.clone()))
        .collect();
    let documents = prepare_documents(input)?;
    let document_hashes: BTreeMap<_, _> = documents
        .iter()
        .map(|(kind, (hash, _))| (kind, hash))
        .collect();
    let manifest_digest = sha256_hex(&json_bytes(&serde_json::json!({
        "nodes": hashes, "documents": document_hashes, "environment_hash": input.environment_hash,
        "image_id": input.image_id, "target": input.target, "sqb_version": input.sqb_version,
        "git_revision": input.git_revision, "git_dirty": input.git_dirty, "git_author": input.git_author,
    }))?);
    Ok(PreparedSnapshot {
        nodes,
        hashes,
        documents,
        manifest_digest,
    })
}

/// Documents are per-snapshot metadata outside node identity: changing one never changes a node
/// hash or records a node change. `columns` holds per-column metadata by node key,
/// `{node_key: {column: {"expression_range": [start, end]}}}`, where `expression_range` is the
/// column's defining expression as `[start, end)` UTF-8 byte offsets of the node's compiled SQL.
/// Readers ignore per-column keys they do not know and treat a missing or malformed range as none.
fn prepare_documents(
    input: &SnapshotInput,
) -> Result<BTreeMap<String, (String, Vec<u8>)>, ControlError> {
    let mut documents: BTreeMap<String, (String, Vec<u8>)> = BTreeMap::new();
    for (kind, value) in &input.documents {
        if ![
            "jobs",
            "groups",
            "triggers",
            "roles",
            "lineage",
            "columns",
            "providers",
        ]
        .contains(&kind.as_str())
        {
            return Err(ControlError::Invalid(format!(
                "unknown snapshot document {kind}"
            )));
        }
        let bytes = json_bytes(value)?;
        documents.insert(kind.clone(), (sha256_hex(&bytes), compress(&bytes)?));
    }
    Ok(documents)
}
