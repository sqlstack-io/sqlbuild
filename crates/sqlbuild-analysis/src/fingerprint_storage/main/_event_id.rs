//! Fixed writer identity excludes timestamps, definitions and future storage columns.

use sha2::{Digest, Sha256};

use crate::fingerprint_storage::models::StoredFingerprint;

/// Stable SHA-256 of the versioned JSON tuple named by the fingerprint storage contract.
pub(super) fn event_id(row: &StoredFingerprint) -> String {
    let identity = serde_json::json!([
        "sqlbuild.fingerprint.event.v1",
        row.node_type,
        row.node_name,
        row.target_database,
        row.target_schema,
        row.target_name,
        row.run_id,
        row.version_hash,
    ]);
    format!("{:x}", Sha256::digest(identity.to_string().as_bytes()))
}
