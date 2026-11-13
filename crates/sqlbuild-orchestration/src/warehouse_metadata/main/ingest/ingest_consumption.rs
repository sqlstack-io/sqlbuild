//! Mirror consumed input versions; rows carry no event identity, so their identity columns derive one.

use serde_json::json;
use uuid::Uuid;

use crate::store::errors::ControlError;
use crate::store::models::ControlStore;
use crate::warehouse_metadata::_helpers::events::ingest_events;
use crate::warehouse_metadata::_helpers::ingestion::batch_records;
use crate::warehouse_metadata::models::{
    MetadataBatch, StateEventImport, WarehouseConsumptionRecord, WarehouseStateLocation,
    WarehouseSyncResponse,
};

pub async fn ingest_consumption(
    store: &ControlStore,
    scope: (Uuid, &str),
    batch: &MetadataBatch,
) -> Result<WarehouseSyncResponse, ControlError> {
    let records: Vec<WarehouseConsumptionRecord> = batch_records(batch)?;
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
                    observed_at: record.recorded_at,
                    columns: Vec::new(),
                    record_json: json!(record),
                })
                .collect(),
            records
                .iter()
                .any(|record| record.state_schema.trim().is_empty()),
        ),
    )
    .await
}
