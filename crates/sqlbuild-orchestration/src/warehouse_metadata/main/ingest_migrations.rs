//! Mirror model-move facts without changing warehouse migration ownership.

use serde_json::json;
use uuid::Uuid;

use crate::store::errors::ControlError;
use crate::store::models::{ControlStore, Parameter};
use crate::warehouse_metadata::_helpers::events::ingest_events;
use crate::warehouse_metadata::_helpers::ingestion::validate_fields;
use crate::warehouse_metadata::models::{
    MigrationsBatch, StateEventImport, StateEventsImport, WarehouseHistoryKind,
    WarehouseStateLocation, WarehouseSyncResponse,
};

pub async fn ingest_migrations(
    store: &ControlStore,
    scope: (Uuid, &str),
    batch: &MigrationsBatch,
) -> Result<WarehouseSyncResponse, ControlError> {
    validate_fields(
        ("migrations", batch.generation, batch.records.len()),
        false,
        false,
    )?;
    ingest_events(
        store,
        scope,
        StateEventsImport {
            kind: WarehouseHistoryKind::Migrations,
            session_id: batch.session_id,
            generation: batch.generation,
            invalid_identity: batch
                .records
                .iter()
                .flat_map(|record| {
                    [
                        &record.event_id,
                        &record.state_schema,
                        &record.destination_model,
                        &record.destination_schema,
                        &record.destination_name,
                    ]
                })
                .any(|value| value.trim().is_empty()),
            records: batch
                .records
                .iter()
                .map(|record| StateEventImport {
                    source_id: record.event_id.clone(),
                    location: WarehouseStateLocation {
                        database: record.state_database.clone(),
                        schema: record.state_schema.clone(),
                    },
                    observed_at: record.created_at,
                    columns: vec![
                        (
                            "target_name",
                            Parameter::OptionalText(record.target_name.clone()),
                        ),
                        (
                            "destination_model",
                            Parameter::Text(record.destination_model.clone()),
                        ),
                        ("decision", Parameter::Text(record.decision.clone())),
                    ],
                    record_json: json!(record),
                })
                .collect(),
            reconciliation: batch.reconciliation.as_ref(),
            payload: json!(batch),
        },
    )
    .await
}
