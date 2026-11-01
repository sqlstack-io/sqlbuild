//! Persist source identities; only legacy rows use a fixed observation-key fallback.

use serde_json::json;
use sqlbuild_state::freshness::main::event_id::event_id as legacy_event_id;
use sqlbuild_state::freshness::models::FreshnessEventIdentity;
use uuid::Uuid;

use crate::store::errors::ControlError;
use crate::store::main::commit::commit;
use crate::store::main::query::query;
use crate::store::models::{Change, ControlStore, Parameter};
use crate::warehouse_metadata::_helpers::ingestion::{begin_ingestion, scoped_event_id};
use crate::warehouse_metadata::constants::MAX_HISTORY_RECORDS;
use crate::warehouse_metadata::models::SourceFreshnessBatch;

pub async fn ingest_source_freshness(
    store: &ControlStore,
    (location_id, target): (Uuid, &str),
    batch: &SourceFreshnessBatch,
) -> Result<bool, ControlError> {
    let invalid_identity = batch
        .records
        .iter()
        .flat_map(|record| {
            [
                &record.state_schema,
                &record.source_name,
                &record.run_id,
                &record.strategy,
                &record.value_kind,
                &record.data_version_hash,
            ]
        })
        .any(|value| value.trim().is_empty());
    let invalid_source_id = batch.records.iter().any(|record| {
        record
            .event_id
            .as_ref()
            .is_some_and(|value| value.trim().is_empty())
    });
    if batch.generation < 1
        || batch.records.len() > MAX_HISTORY_RECORDS
        || invalid_identity
        || invalid_source_id
    {
        return Err(ControlError::Invalid(format!(
            "source freshness sync needs a positive generation, at most {MAX_HISTORY_RECORDS} records and nonempty state identities",
        )));
    }
    let mut transaction = begin_ingestion(
        store,
        (location_id, target),
        (batch.session_id, batch.generation),
    )
    .await?;
    let mut applied = false;
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
        let received_at = transaction.now;
        let inserted;
        (transaction, inserted) = query(transaction,
            "INSERT INTO sqb_warehouse_freshness (event_id,location_id,target,source_event_id,observed_at,record_json,received_at) \
             VALUES ($1,$2,$3,$4,$5,$6,$7) ON CONFLICT (event_id) DO NOTHING RETURNING event_id",
            vec![Parameter::Id(event_id), Parameter::Id(location_id), Parameter::Text(target.into()),
                Parameter::Text(source_event_id), Parameter::Time(record.observed_at), Parameter::Json(json!(record)), Parameter::Time(received_at)]).await?;
        applied |= !inserted.is_empty();
    }
    if applied {
        transaction
            .changes
            .insert(Change::NodeStatus(location_id, target.into()));
    }
    commit(transaction).await?;
    Ok(applied)
}
