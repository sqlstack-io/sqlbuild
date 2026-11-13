//! Mirror display-only estimated query cost observations; each refresh is another observation.

use serde_json::json;
use uuid::Uuid;

use crate::store::errors::ControlError;
use crate::store::models::{ControlStore, Parameter};
use crate::warehouse_metadata::_helpers::events::ingest_events;
use crate::warehouse_metadata::_helpers::ingestion::batch_records;
use crate::warehouse_metadata::models::{
    MetadataBatch, StateEventImport, WarehouseResourceCostRecord, WarehouseStateLocation,
    WarehouseSyncResponse,
};

pub async fn ingest_resource_costs(
    store: &ControlStore,
    scope: (Uuid, &str),
    batch: &MetadataBatch,
) -> Result<WarehouseSyncResponse, ControlError> {
    let records: Vec<WarehouseResourceCostRecord> = batch_records(batch)?;
    let invalid_identity: bool = records.iter().any(invalid_identity);
    ingest_events(
        store,
        scope,
        batch,
        (
            records
                .iter()
                .map(|record| StateEventImport {
                    source_id: record.observation_id.clone(),
                    location: WarehouseStateLocation {
                        database: record.state_database.clone(),
                        schema: record.state_schema.clone(),
                    },
                    observed_at: record.observed_at,
                    columns: vec![
                        ("run_id", Parameter::Text(record.run_id.clone())),
                        (
                            "orchestration_run_id",
                            Parameter::OptionalText(record.orchestration_run_id.clone()),
                        ),
                        ("attempt", Parameter::Integer(record.attempt)),
                        (
                            "resource_type",
                            Parameter::Text(record.resource_type.clone()),
                        ),
                        (
                            "resource_name",
                            Parameter::Text(record.resource_name.clone()),
                        ),
                        ("run_started_at", Parameter::Time(record.run_started_at)),
                    ],
                    record_json: json!(record),
                })
                .collect(),
            invalid_identity,
        ),
    )
    .await
}

/// Whether an observation lacks an attempt or any part of its identity.
fn invalid_identity(record: &WarehouseResourceCostRecord) -> bool {
    let identity: [&String; 5] = [
        &record.observation_id,
        &record.state_schema,
        &record.run_id,
        &record.resource_type,
        &record.resource_name,
    ];
    record.attempt < 1 || identity.iter().any(|value| value.trim().is_empty())
}
