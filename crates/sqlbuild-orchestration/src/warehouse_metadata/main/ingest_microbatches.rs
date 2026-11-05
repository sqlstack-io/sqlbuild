//! Mirror append-only microbatch facts without reading or changing execution coordination.

use serde_json::json;
use uuid::Uuid;

use crate::store::errors::ControlError;
use crate::store::models::{ControlStore, Parameter};
use crate::warehouse_metadata::_helpers::events::ingest_events;
use crate::warehouse_metadata::_helpers::ingestion::batch_records;
use crate::warehouse_metadata::models::{
    MetadataBatch, StateEventImport, WarehouseMicrobatchRecord, WarehouseStateLocation,
    WarehouseSyncResponse,
};

pub async fn ingest_microbatches(
    store: &ControlStore,
    scope: (Uuid, &str),
    batch: &MetadataBatch,
) -> Result<WarehouseSyncResponse, ControlError> {
    let records: Vec<WarehouseMicrobatchRecord> = batch_records(batch)?;
    ingest_events(
        store,
        scope,
        batch,
        (
            records
                .iter()
                .map(|record| StateEventImport {
                    source_id: record.event_id.clone(),
                    location: WarehouseStateLocation {
                        database: record.state_database.clone(),
                        schema: record.state_schema.clone(),
                    },
                    observed_at: record.created_at,
                    columns: vec![
                        ("model_name", Parameter::Text(record.model_name.clone())),
                        ("target_name", Parameter::Text(record.target_name.clone())),
                        ("scope_kind", Parameter::Text(record.scope_kind.clone())),
                        ("scope_key", Parameter::Text(record.scope_key.clone())),
                        (
                            "physical_generation_id",
                            Parameter::Text(record.physical_generation_id.clone()),
                        ),
                        ("record_type", Parameter::Text(record.record_type.clone())),
                        (
                            "origin_run_id",
                            Parameter::Text(record.origin_run_id.clone()),
                        ),
                        (
                            "execution_run_id",
                            Parameter::Text(record.execution_run_id.clone()),
                        ),
                        (
                            "replay_requirement_id",
                            Parameter::OptionalText(record.replay_requirement_id.clone()),
                        ),
                    ],
                    record_json: json!(record),
                })
                .collect(),
            records
                .iter()
                .flat_map(|record| {
                    [
                        &record.event_id,
                        &record.state_schema,
                        &record.scope_kind,
                        &record.scope_key,
                        &record.model_name,
                        &record.target_name,
                        &record.physical_generation_id,
                        &record.record_type,
                    ]
                })
                .any(|value| value.trim().is_empty()),
        ),
    )
    .await
}
