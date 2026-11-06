//! Fixed legacy fingerprint keys and validation of bounded source history pages.

use serde_json::json;
use sqlbuild_recall::records::main::sha256_hex::sha256_hex;

use crate::store::errors::ControlError;
use crate::warehouse_metadata::_helpers::ingestion::{outside_scan, validate_fields};
use crate::warehouse_metadata::models::{MetadataBatch, WarehouseFingerprintRecord};

pub(crate) fn validate_batch(
    batch: &MetadataBatch,
    records: &[WarehouseFingerprintRecord],
) -> Result<(), ControlError> {
    let invalid = records
        .iter()
        .flat_map(|record| {
            [
                &record.state_schema,
                &record.node_type,
                &record.node_name,
                &record.run_id,
            ]
        })
        .any(|value| value.trim().is_empty());
    let invalid_id = records
        .iter()
        .filter_map(|record| record.event_id.as_deref())
        .any(|value| value.trim().is_empty());
    validate_fields(
        batch,
        invalid || invalid_id,
        outside_scan(
            batch,
            records
                .iter()
                .map(|record| (&record.state_database, &record.state_schema)),
        ),
    )
}

/// Hash fixed predecessor fields only when the warehouse has no persisted identity.
pub(crate) fn source_event_id(record: &WarehouseFingerprintRecord) -> String {
    if let Some(event_id) = &record.event_id {
        return event_id.clone();
    }
    sha256_hex(
        json!([
            "sqlbuild.fingerprint.legacy.v1",
            record.node_type,
            record.node_name,
            record.target_database,
            record.target_schema,
            record.target_name,
            record.run_id,
            record.definition_hash,
            record.version_hash,
            record.schema_fingerprint,
            record.definition,
            record.metadata_json,
            record.ts.format("%Y-%m-%dT%H:%M:%S%.6f+00:00").to_string(),
        ])
        .to_string()
        .as_bytes(),
    )
}
