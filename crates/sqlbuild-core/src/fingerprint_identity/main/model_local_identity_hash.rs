//! A model's local identity hash from its query fingerprint and non-query metadata.

use crate::fingerprint_identity::main::stable_identity_hash::stable_identity_hash;

/// The hash of the query fingerprint and metadata JSON, one per line.
pub fn model_local_identity_hash(query_fingerprint: &str, metadata_json: &str) -> String {
    stable_identity_hash(&format!("{query_fingerprint}\n{metadata_json}"))
}
