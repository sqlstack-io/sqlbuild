//! Mirror display-only build observations; planning reads the warehouse, never these rows.

use serde_json::json;
use uuid::Uuid;

use crate::store::errors::ControlError;
use crate::store::models::{ControlStore, Parameter};
use crate::warehouse_metadata::_helpers::events::ingest_events;
use crate::warehouse_metadata::_helpers::ingestion::batch_records;
use crate::warehouse_metadata::models::{
    MetadataBatch, StateEventImport, WarehouseObservationRecord, WarehouseStateLocation,
    WarehouseSyncResponse,
};

pub async fn ingest_observations(
    store: &ControlStore,
    scope: (Uuid, &str),
    batch: &MetadataBatch,
) -> Result<WarehouseSyncResponse, ControlError> {
    let records: Vec<WarehouseObservationRecord> = batch_records(batch)?;
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
                    observed_at: record.observed_at,
                    columns: vec![
                        ("kind", Parameter::Text(record.kind.clone())),
                        ("node", Parameter::Text(record.node.clone())),
                    ],
                    record_json: json!(record),
                })
                .collect(),
            records
                .iter()
                .flat_map(|record| {
                    [
                        &record.observation_id,
                        &record.state_schema,
                        &record.kind,
                        &record.node,
                        &record.run_id,
                    ]
                })
                .any(|value| value.trim().is_empty()),
        ),
    )
    .await
}
