//! Mirror compatibility-view history, never an assertion about current relation existence.

use serde_json::json;
use uuid::Uuid;

use crate::store::errors::ControlError;
use crate::store::models::{ControlStore, Parameter};
use crate::warehouse_metadata::_helpers::events::ingest_events;
use crate::warehouse_metadata::_helpers::ingestion::validate_fields;
use crate::warehouse_metadata::models::{
    OldNameViewsBatch, StateEventImport, StateEventsImport, WarehouseHistoryKind,
    WarehouseStateLocation, WarehouseSyncResponse,
};

pub async fn ingest_old_name_views(
    store: &ControlStore,
    scope: (Uuid, &str),
    batch: &OldNameViewsBatch,
) -> Result<WarehouseSyncResponse, ControlError> {
    validate_fields(
        ("old name views", batch.generation, batch.records.len()),
        false,
        false,
    )?;
    ingest_events(
        store,
        scope,
        StateEventsImport {
            kind: WarehouseHistoryKind::OldNameViews,
            session_id: batch.session_id,
            generation: batch.generation,
            invalid_identity: batch
                .records
                .iter()
                .flat_map(|record| {
                    [
                        &record.event_id,
                        &record.state_schema,
                        &record.migration_event_id,
                        &record.destination_model,
                        &record.new_schema,
                        &record.new_name,
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
                            "migration_event_id",
                            Parameter::Text(record.migration_event_id.clone()),
                        ),
                        (
                            "destination_model",
                            Parameter::Text(record.destination_model.clone()),
                        ),
                        ("event_type", Parameter::Text(record.event_type.clone())),
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
