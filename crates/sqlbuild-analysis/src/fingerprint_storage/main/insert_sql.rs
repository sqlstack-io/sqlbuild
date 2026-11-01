//! Append-only fingerprint inserts.

use crate::fingerprint_storage::_helpers::base64::encode;
use crate::fingerprint_storage::_helpers::literals::{optional_string_literal, string_literal};
use crate::fingerprint_storage::_helpers::table_columns::column_list;
use crate::fingerprint_storage::constants::COLUMN_EVENT_ID;
use crate::fingerprint_storage::errors::FingerprintStorageError;
use crate::fingerprint_storage::main::_event_id::event_id;
use crate::fingerprint_storage::models::StoredFingerprint;

/// One multi-row `INSERT` for a non-empty batch.
pub fn insert_sql(
    qualified_name: &str,
    fingerprints: &[StoredFingerprint],
) -> Result<String, FingerprintStorageError> {
    if fingerprints.is_empty() {
        return Err(FingerprintStorageError::EmptyInsert);
    }
    let rows: Vec<String> = fingerprints.iter().map(row_values).collect();
    Ok(format!(
        "INSERT INTO {qualified_name} ({}, {COLUMN_EVENT_ID}) VALUES {}",
        column_list(),
        rows.join(", ")
    ))
}

fn row_values(fingerprint: &StoredFingerprint) -> String {
    let values: [String; 13] = [
        string_literal(&fingerprint.node_type),
        string_literal(&fingerprint.node_name),
        optional_string_literal(fingerprint.target_database.as_deref()),
        optional_string_literal(fingerprint.target_schema.as_deref()),
        optional_string_literal(fingerprint.target_name.as_deref()),
        format!("'{}'", fingerprint.run_id),
        format!("'{}'", fingerprint.definition_hash),
        format!("'{}'", fingerprint.version_hash),
        format!("'{}'", fingerprint.schema_fingerprint),
        format!("'{}'", encode(fingerprint.definition.as_bytes())),
        format!("'{}'", encode(fingerprint.metadata_json.as_bytes())),
        format!("'{}'", fingerprint.ts),
        string_literal(&event_id(fingerprint)),
    ];
    format!("({})", values.join(", "))
}
