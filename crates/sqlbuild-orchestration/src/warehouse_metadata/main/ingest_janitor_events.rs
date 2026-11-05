//! Mirror archive/delete history; the warehouse remains the owner of physical cleanup.

use serde_json::json;
use uuid::Uuid;

use crate::store::errors::ControlError;
use crate::store::models::{ControlStore, Parameter};
use crate::warehouse_metadata::_helpers::events::ingest_events;
use crate::warehouse_metadata::_helpers::ingestion::batch_records;
use crate::warehouse_metadata::models::{
    MetadataBatch, StateEventImport, WarehouseJanitorRecord, WarehouseStateLocation,
    WarehouseSyncResponse,
};

pub async fn ingest_janitor_events(
    store: &ControlStore,
    scope: (Uuid, &str),
    batch: &MetadataBatch,
) -> Result<WarehouseSyncResponse, ControlError> {
    let records: Vec<WarehouseJanitorRecord> = batch_records(batch)?;
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
                    observed_at: record.occurred_at,
                    columns: vec![
                        ("event_type", Parameter::Text(record.event_type.clone())),
                        (
                            "relation_database",
                            Parameter::OptionalText(record.relation_database.clone()),
                        ),
                        (
                            "relation_schema",
                            Parameter::Text(record.relation_schema.clone()),
                        ),
                        ("archive_name", Parameter::Text(record.archive_name.clone())),
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
                        &record.relation_schema,
                        &record.event_type,
                        &record.archive_name,
                        &record.archive_qualified_name,
                    ]
                })
                .any(|value| value.trim().is_empty()),
        ),
    )
    .await
}
