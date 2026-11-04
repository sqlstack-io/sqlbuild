//! Mirror consumed-input history for inspection; run decisions never read this mirror.

use serde_json::json;
use uuid::Uuid;

use crate::store::errors::ControlError;
use crate::store::models::{ControlStore, Parameter};
use crate::warehouse_metadata::_helpers::events::ingest_events;
use crate::warehouse_metadata::_helpers::ingestion::validate_fields;
use crate::warehouse_metadata::models::{
    ConsumptionBatch, StateEventImport, StateEventsImport, WarehouseHistoryKind,
    WarehouseStateLocation, WarehouseSyncResponse,
};

pub async fn ingest_consumption(
    store: &ControlStore,
    scope: (Uuid, &str),
    batch: &ConsumptionBatch,
) -> Result<WarehouseSyncResponse, ControlError> {
    validate_fields(
        ("consumption", batch.generation, batch.records.len()),
        false,
        false,
    )?;
    ingest_events(
        store,
        scope,
        StateEventsImport {
            kind: WarehouseHistoryKind::Consumption,
            session_id: batch.session_id,
            generation: batch.generation,
            invalid_identity: batch
                .records
                .iter()
                .flat_map(|record| {
                    [
                        &record.event_id,
                        &record.state_schema,
                        &record.execution_id,
                        &record.phase,
                        &record.node_name,
                        &record.target_schema,
                        &record.target_name,
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
                    observed_at: record.recorded_at,
                    columns: vec![
                        ("execution_id", Parameter::Text(record.execution_id.clone())),
                        ("phase", Parameter::Text(record.phase.clone())),
                        ("run_id", Parameter::Text(record.run_id.clone())),
                        ("node_name", Parameter::Text(record.node_name.clone())),
                        (
                            "target_database",
                            Parameter::OptionalText(record.target_database.clone()),
                        ),
                        (
                            "target_schema",
                            Parameter::Text(record.target_schema.clone()),
                        ),
                        ("target_name", Parameter::Text(record.target_name.clone())),
                        ("version_hash", Parameter::Text(record.version_hash.clone())),
                        ("input_count", Parameter::Integer(record.input_count)),
                        (
                            "input_kind",
                            Parameter::OptionalText(record.input_kind.clone()),
                        ),
                        (
                            "input_name",
                            Parameter::OptionalText(record.input_name.clone()),
                        ),
                        (
                            "input_version",
                            Parameter::OptionalText(record.input_version.clone()),
                        ),
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
