//! Import warehouse-owned check results without deriving scheduler execution or recovery.

use serde_json::json;
use uuid::Uuid;

use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlStore, Parameter};
use crate::warehouse_metadata::_helpers::ingestion::{
    begin_ingestion, commit_ingestion, scoped_event_id, validate_fields,
};
use crate::warehouse_metadata::_helpers::reconciliation::begin_reconciliation;
use crate::warehouse_metadata::models::{AuditResultsBatch, WarehouseSyncResponse};

pub async fn ingest_audit_results(
    store: &ControlStore,
    (location_id, target): (Uuid, &str),
    batch: &AuditResultsBatch,
) -> Result<WarehouseSyncResponse, ControlError> {
    validate_fields(
        ("audit results", batch.generation, batch.records.len()),
        batch
            .records
            .iter()
            .flat_map(|record| [&record.result_id, &record.state_schema, &record.audit_name])
            .any(|value| value.trim().is_empty()),
        batch.reconciliation.as_ref().is_some_and(|page| {
            batch.records.iter().any(|record| {
                record.state_database != page.state_database
                    || record.state_schema != page.state_schema
                    || page
                        .day
                        .is_some_and(|day| record.occurred_at.date_naive() != day)
            })
        }),
    )?;
    let mut transaction = begin_ingestion(
        store,
        (location_id, target),
        (batch.session_id, batch.generation),
    )
    .await?;
    if let Some(page) = &batch.reconciliation {
        transaction = begin_reconciliation(
            transaction,
            ("sqb_warehouse_audit_results", location_id, target),
            page,
            json!(batch),
        )
        .await?;
    }
    let mut applied = false;
    let mut keys: Vec<Uuid> = Vec::new();
    for record in &batch.records {
        let event_id = scoped_event_id(
            (location_id, target),
            (
                record.state_database.as_deref(),
                &record.state_schema,
                "_sqlbuild_audit_results",
            ),
            &record.result_id,
        )?;
        if batch.reconciliation.is_some() {
            keys.push(event_id);
        }
        let now = transaction.now;
        let inserted;
        (transaction, inserted) = query(transaction,
            "INSERT INTO sqb_warehouse_audit_results (event_id,location_id,target,source_event_id,observed_at,state_database,state_schema,audit_name,outcome,record_json,received_at) \
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11) ON CONFLICT (event_id) DO NOTHING RETURNING event_id",
            vec![Parameter::Id(event_id), Parameter::Id(location_id), Parameter::Text(target.into()),
                Parameter::Text(record.result_id.clone()), Parameter::Time(record.occurred_at),
                Parameter::OptionalText(record.state_database.clone()), Parameter::Text(record.state_schema.clone()),
                Parameter::Text(record.audit_name.clone()), Parameter::Text(record.outcome.clone()),
                Parameter::Json(json!(record)), Parameter::Time(now)]).await?;
        applied |= !inserted.is_empty();
    }
    commit_ingestion(
        transaction,
        ("sqb_warehouse_audit_results", location_id, target),
        (batch.reconciliation.as_ref(), &keys),
        applied,
    )
    .await
}
