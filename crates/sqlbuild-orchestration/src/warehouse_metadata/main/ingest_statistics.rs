//! First committed metadata sample per relation and UTC hour wins; samples are not change facts.

use std::collections::BTreeSet;

use serde_json::json;
use uuid::Uuid;

use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlStore, Parameter};
use crate::warehouse_metadata::_helpers::ingestion::{
    batch_records, begin_ingestion, commit_ingestion, validate_fields,
};
use crate::warehouse_metadata::_helpers::statistics::statistics_event_id;
use crate::warehouse_metadata::models::{
    MetadataBatch, WarehouseStatisticsRecord, WarehouseSyncResponse,
};

pub async fn ingest_statistics(
    store: &ControlStore,
    scope: (Uuid, &str),
    batch: &MetadataBatch,
) -> Result<WarehouseSyncResponse, ControlError> {
    let records: Vec<WarehouseStatisticsRecord> = batch_records(batch)?;
    let mut names: BTreeSet<_> = BTreeSet::new();
    validate_fields(
        batch,
        records.iter().any(|record| {
            record.state_schema.trim().is_empty()
                || record.name.trim().is_empty()
                || record.byte_count.is_some() != record.byte_count_basis.is_some()
                || record
                    .byte_count_basis
                    .as_ref()
                    .is_some_and(|basis| basis.trim().is_empty())
                || !names.insert((
                    (
                        record.state_database.as_deref(),
                        &record.state_schema,
                        &record.name,
                    ),
                    record.observed_at.timestamp().div_euclid(3600),
                ))
        }),
        batch.scan.is_some(),
    )?;
    let mut transaction =
        begin_ingestion(store, scope, (batch.session_id, batch.generation)).await?;
    let mut applied = false;
    for record in &records {
        let event_id = statistics_event_id(
            scope,
            (
                record.state_database.as_deref(),
                &record.state_schema,
                &record.name,
            ),
            record.observed_at,
        )?;
        let now = transaction.now;
        let rows;
        (transaction, rows) = query(transaction,
            "INSERT INTO sqb_warehouse_statistics (event_id,location_id,target,state_database,state_schema,relation_name,observed_at,record_json,received_at) \
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9) ON CONFLICT (event_id) DO NOTHING RETURNING event_id",
            vec![Parameter::Id(event_id),Parameter::Id(scope.0),Parameter::Text(scope.1.into()),
                Parameter::OptionalText(record.state_database.clone()),Parameter::Text(record.state_schema.clone()),
                Parameter::Text(record.name.clone()),Parameter::Time(record.observed_at),Parameter::Json(json!(record)),Parameter::Time(now)]).await?;
        applied |= !rows.is_empty();
    }
    commit_ingestion(
        transaction,
        ("sqb_warehouse_statistics", scope.0, scope.1),
        (None, &[]),
        applied,
    )
    .await
}
