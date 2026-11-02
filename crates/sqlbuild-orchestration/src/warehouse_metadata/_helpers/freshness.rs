//! Validate bounded freshness observations and their optional reconciliation scope.

use crate::store::errors::ControlError;
use crate::warehouse_metadata::_helpers::ingestion::validate_fields;
use crate::warehouse_metadata::models::SourceFreshnessBatch;

pub(crate) fn validate_batch(batch: &SourceFreshnessBatch) -> Result<(), ControlError> {
    let invalid_identity = batch
        .records
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
    let invalid_source_id = batch
        .records
        .iter()
        .filter_map(|record| record.event_id.as_deref())
        .any(|value| value.trim().is_empty());
    let invalid_scope = batch.reconciliation.as_ref().is_some_and(|page| {
        batch.records.iter().any(|record| {
            record.state_database != page.state_database
                || record.state_schema != page.state_schema
                || record.observed_at.date_naive() != page.day
        })
    });
    validate_fields(
        ("source freshness", batch.generation, batch.records.len()),
        invalid_identity || invalid_source_id,
        invalid_scope,
    )
}
