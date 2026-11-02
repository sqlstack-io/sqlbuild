//! Persist source identities; only legacy rows use a fixed observation-key fallback.

use serde_json::json;
use sqlbuild_state::freshness::main::event_id::event_id as legacy_event_id;
use sqlbuild_state::freshness::models::FreshnessEventIdentity;
use uuid::Uuid;

use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlStore, Parameter};
use crate::warehouse_metadata::_helpers::freshness::validate_batch;
use crate::warehouse_metadata::_helpers::ingestion::{
    begin_ingestion, commit_ingestion, scoped_event_id,
};
use crate::warehouse_metadata::_helpers::reconciliation::begin_reconciliation;
use crate::warehouse_metadata::models::{SourceFreshnessBatch, WarehouseSyncResponse};

pub async fn ingest_source_freshness(
    store: &ControlStore,
    (location_id, target): (Uuid, &str),
    batch: &SourceFreshnessBatch,
) -> Result<WarehouseSyncResponse, ControlError> {
    validate_batch(batch)?;
    let mut transaction = begin_ingestion(
        store,
        (location_id, target),
        (batch.session_id, batch.generation),
    )
    .await?;
    if let Some(page) = &batch.reconciliation {
        transaction = begin_reconciliation(
            transaction,
            ("sqb_warehouse_freshness", location_id, target),
            page,
            json!(batch),
        )
        .await?;
    }
    let mut applied = false;
    let mut keys: Vec<Uuid> = Vec::new();
    for record in &batch.records {
        let source_event_id = record.event_id.clone().unwrap_or_else(|| {
            legacy_event_id(&FreshnessEventIdentity {
                source_name: &record.source_name,
                target_database: record.target_database.as_deref(),
                target_schema: record.target_schema.as_deref(),
                target_name: record.target_name.as_deref(),
                run_id: &record.run_id,
                strategy: &record.strategy,
                value_kind: &record.value_kind,
                data_version_hash: &record.data_version_hash,
                observed_at: &record
                    .observed_at
                    .format("%Y-%m-%dT%H:%M:%S%.6f+00:00")
                    .to_string(),
            })
        });
        let event_id = scoped_event_id(
            (location_id, target),
            (
                record.state_database.as_deref(),
                &record.state_schema,
                "_sqlbuild_source_freshness",
            ),
            &source_event_id,
        )?;
        if batch.reconciliation.is_some() {
            keys.push(event_id);
        }
        let received_at = transaction.now;
        let inserted;
        (transaction, inserted) = query(transaction,
            "INSERT INTO sqb_warehouse_freshness (event_id,location_id,target,source_event_id,observed_at,record_json,received_at,state_database,state_schema) \
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9) ON CONFLICT (event_id) DO NOTHING RETURNING event_id",
            vec![Parameter::Id(event_id), Parameter::Id(location_id), Parameter::Text(target.into()),
                Parameter::Text(source_event_id), Parameter::Time(record.observed_at), Parameter::Json(json!(record)), Parameter::Time(received_at),
                Parameter::OptionalText(record.state_database.clone()), Parameter::Text(record.state_schema.clone())]).await?;
        applied |= !inserted.is_empty();
    }
    commit_ingestion(
        transaction,
        ("sqb_warehouse_freshness", location_id, target),
        (batch.reconciliation.as_ref(), &keys),
        applied,
    )
    .await
}
