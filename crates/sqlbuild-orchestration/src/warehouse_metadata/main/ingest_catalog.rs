//! A full catalog scan records structural changes, never inferred relation existence.

use serde_json::json;
use uuid::Uuid;

use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlStore, Parameter};
use crate::warehouse_metadata::_helpers::catalog::{
    append_catalog_fact, project_catalog_record, validate_catalog,
};
use crate::warehouse_metadata::_helpers::ingestion::{
    begin_ingestion, commit_ingestion, scoped_event_id,
};
use crate::warehouse_metadata::_helpers::reconciliation::begin_reconciliation;
use crate::warehouse_metadata::models::{CatalogBatch, WarehouseSyncResponse};

pub async fn ingest_catalog(
    store: &ControlStore,
    scope: (Uuid, &str),
    batch: &CatalogBatch,
) -> Result<WarehouseSyncResponse, ControlError> {
    validate_catalog(batch)?;
    let page = &batch.reconciliation;
    let transaction = begin_ingestion(store, scope, (batch.session_id, batch.generation)).await?;
    let (mut transaction, received) = begin_reconciliation(
        transaction,
        ("sqb_warehouse_catalog", scope.0, scope.1),
        page,
        json!(batch),
    )
    .await?;
    let mut keys: Vec<Uuid> = Vec::new();
    let mut applied = false;
    if !received {
        for record in &batch.records {
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
            let record_json = json!(record);
            if existing
                .first()
                .is_some_and(|row| row["record_json"] == record_json)
            {
                continue;
            }
            transaction = append_catalog_fact(
                transaction,
                (page.scan_id, page.page, relation_id),
                scope,
                (record_json.clone(), true),
            )
            .await?;
            transaction = project_catalog_record(transaction, scope, relation_id, record).await?;
            applied = true;
        }
    }
    commit_ingestion(
        transaction,
        ("sqb_warehouse_catalog", scope.0, scope.1),
        (Some(page), &keys),
        applied,
    )
    .await
}
