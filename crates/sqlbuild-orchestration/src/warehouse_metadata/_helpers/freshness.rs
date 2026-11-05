//! Validate source observations before they are stored.

use crate::store::errors::ControlError;
use crate::warehouse_metadata::_helpers::ingestion::{outside_scan, validate_fields};
use crate::warehouse_metadata::models::{MetadataBatch, WarehouseFreshnessRecord};

pub(crate) fn validate_batch(
    batch: &MetadataBatch,
    records: &[WarehouseFreshnessRecord],
) -> Result<(), ControlError> {
    let invalid_identity = records
        .iter()
        .flat_map(|record| {
            [
                &record.state_schema,
                &record.source_name,
                &record.run_id,
                &record.strategy,
                &record.value_kind,
                &record.data_version_hash,
            ]
        })
        .any(|value| value.trim().is_empty());
    let invalid_source_id = records
        .iter()
        .filter_map(|record| record.event_id.as_deref())
        .any(|value| value.trim().is_empty());
    validate_fields(
        batch,
        invalid_identity || invalid_source_id,
        outside_scan(
            batch,
            records
                .iter()
                .map(|record| (&record.state_database, &record.state_schema)),
        ),
    )
}
