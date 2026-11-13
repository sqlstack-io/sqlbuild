//! Mirror column rename decisions; the warehouse state table stays the owner of the history.

use serde_json::json;
use uuid::Uuid;

use crate::store::errors::ControlError;
use crate::store::models::ControlStore;
use crate::warehouse_metadata::_helpers::events::ingest_events;
use crate::warehouse_metadata::_helpers::ingestion::batch_records;
use crate::warehouse_metadata::models::{
    MetadataBatch, StateEventImport, WarehouseColumnMigrationRecord, WarehouseStateLocation,
    WarehouseSyncResponse,
};

pub async fn ingest_column_migrations(
    store: &ControlStore,
    scope: (Uuid, &str),
    batch: &MetadataBatch,
) -> Result<WarehouseSyncResponse, ControlError> {
    let records: Vec<WarehouseColumnMigrationRecord> = batch_records(batch)?;
    ingest_events(
        store,
        scope,
        batch,
        (
            records
                .iter()
                .map(|record| StateEventImport {
                    location: WarehouseStateLocation {
                        database: record.state_database.clone(),
                        schema: record.state_schema.clone(),
                    },
                    observed_at: record.created_at,
                    columns: Vec::new(),
                    record_json: json!(record),
                })
                .collect(),
            records
                .iter()
                .flat_map(|record| [&record.event_id, &record.state_schema])
                .any(|value| value.trim().is_empty()),
        ),
    )
    .await
}
