//! Observation identity excludes mutable payload and future storage columns.

use sha2::{Digest, Sha256};

use crate::freshness::models::FreshnessEventIdentity;

/// `observed_at` is UTC ISO text with six fractional digits and a `+00:00` offset.
pub fn event_id(row: &FreshnessEventIdentity<'_>) -> String {
    let identity = serde_json::json!([
        "sqlbuild.source-freshness.event.v1",
        row.source_name,
        row.target_database,
        row.target_schema,
        row.target_name,
        row.run_id,
        row.observed_at,
        row.strategy,
        row.value_kind,
        row.data_version_hash,
    ]);
    format!("{:x}", Sha256::digest(identity.to_string().as_bytes()))
}
