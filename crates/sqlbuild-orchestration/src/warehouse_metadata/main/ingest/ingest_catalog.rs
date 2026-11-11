//! A full catalog scan replaces its namespace, never inferring relation existence otherwise.

use serde_json::json;
use uuid::Uuid;

use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlStore, Parameter};
use crate::warehouse_metadata::_helpers::catalog::{project_catalog_record, validate_catalog};
use crate::warehouse_metadata::_helpers::ingestion::{
    batch_records, begin_ingestion, commit_ingestion, scoped_event_id,
};
use crate::warehouse_metadata::constants::CATALOG_MIRROR;
use crate::warehouse_metadata::models::{
    MetadataBatch, WarehouseCatalogRecord, WarehouseSyncResponse,
};

pub async fn ingest_catalog(
    store: &ControlStore,
    scope: (Uuid, &str),
    batch: &MetadataBatch,
) -> Result<WarehouseSyncResponse, ControlError> {
    let records: Vec<WarehouseCatalogRecord> = batch_records(batch)?;
    validate_catalog(batch, &records)?;
    let mut transaction =
        begin_ingestion(store, scope, (batch.session_id, batch.generation)).await?;
    let mut keys: Vec<Uuid> = Vec::new();
    let mut applied = false;
    for record in &records {
        let relation_id = scoped_event_id(
            scope,
            (
                record.state_database.as_deref(),
                &record.state_schema,
                "catalog",
            ),
            &record.name,
        )?;
        keys.push(relation_id);
        let existing;
        (transaction, existing) = query(
            transaction,
            "SELECT record_json FROM sqb_warehouse_catalog WHERE event_id=$1",
            vec![Parameter::Id(relation_id)],
        )
        .await?;
        if existing
            .first()
            .is_some_and(|row| row["record_json"] == json!(record))
        {
            continue;
        }
        transaction = project_catalog_record(transaction, scope, relation_id, record).await?;
        applied = true;
    }
    commit_ingestion(
        transaction,
        (CATALOG_MIRROR, scope.0, scope.1),
        (batch.scan.as_ref(), &keys),
        applied,
    )
    .await
}
