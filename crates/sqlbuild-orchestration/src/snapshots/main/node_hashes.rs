//! The content hash of each compiled node, as a publication pins it.

use std::collections::BTreeMap;

use sqlbuild_recall::records::main::sha256_hex::sha256_hex;

use crate::snapshots::_helpers::content::node_content;
use crate::snapshots::models::SnapshotInput;
use crate::store::errors::ControlError;

/// Each node's hash over its execution content, the same hash a publication records for it.
pub fn node_hashes(input: &SnapshotInput) -> Result<BTreeMap<String, String>, ControlError> {
    input
        .nodes
        .iter()
        .map(|(key, node)| Ok((key.clone(), sha256_hex(&node_content(node)?.definition))))
        .collect()
}
