//! Import warehouse-owned check results without deriving scheduler execution or recovery.

use serde_json::json;
use uuid::Uuid;

use crate::store::errors::ControlError;
use crate::store::models::{ControlStore, Parameter};
use crate::warehouse_metadata::_helpers::events::ingest_events;
use crate::warehouse_metadata::_helpers::ingestion::batch_records;
use crate::warehouse_metadata::models::{
    MetadataBatch, StateEventImport, WarehouseAuditRecord, WarehouseStateLocation,
    WarehouseSyncResponse,
};

pub async fn ingest_audit_results(
    store: &ControlStore,
    scope: (Uuid, &str),
    batch: &MetadataBatch,
) -> Result<WarehouseSyncResponse, ControlError> {
    let records: Vec<WarehouseAuditRecord> = batch_records(batch)?;
    ingest_events(
        store,
        scope,
        batch,
        (
            records
                .iter()
                .map(|record| StateEventImport {
                    source_id: record.result_id.clone(),
                    location: WarehouseStateLocation {
                        database: record.state_database.clone(),
                        schema: record.state_schema.clone(),
                    },
                    observed_at: record.occurred_at,
                    columns: vec![
                        ("audit_name", Parameter::Text(record.audit_name.clone())),
                        ("outcome", Parameter::Text(record.outcome.clone())),
                    ],
                    record_json: json!(record),
                })
                .collect(),
            records
                .iter()
                .flat_map(|record| [&record.result_id, &record.state_schema, &record.audit_name])
                .any(|value| value.trim().is_empty()),
        ),
    )
    .await
}
