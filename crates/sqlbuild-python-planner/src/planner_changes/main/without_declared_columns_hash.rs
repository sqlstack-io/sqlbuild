//! Fingerprint metadata JSON without the declared-columns hash key, if present.

use crate::planner_changes::_helpers::metadata;

/// Fingerprint metadata JSON without the declared-columns hash key, if present.
#[inline]
pub fn without_declared_columns_hash(metadata_json: &str) -> String {
    metadata::without_declared_columns_hash(metadata_json)
}
