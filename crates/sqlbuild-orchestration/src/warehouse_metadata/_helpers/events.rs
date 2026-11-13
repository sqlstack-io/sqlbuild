//! One fenced, atomic persistence path for immutable warehouse events, keyed by their source columns.

use std::collections::HashSet;

use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use sqlbuild_recall::records::main::sha256_hex::sha256_hex;
use uuid::Uuid;

use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::main::query_records::query_records;
use crate::store::models::{ControlStore, ControlTransaction, Parameter};
use crate::warehouse_metadata::_helpers::ingestion::{
    begin_ingestion, commit_ingestion, outside_scan, scoped_event_id, validate_fields,
};
use crate::warehouse_metadata::models::{
    MetadataBatch, MetadataKeyPage, StateEventImport, WarehouseHistoryKind, WarehouseSyncResponse,
};

pub(crate) async fn ingest_events(
    store: &ControlStore,
    (location_id, target): (Uuid, &str),
    batch: &MetadataBatch,
    (records, invalid_identity): (Vec<StateEventImport>, bool),
) -> Result<WarehouseSyncResponse, ControlError> {
    validate_fields(
        batch,
        invalid_identity,
        outside_scan(
            batch,
            records
                .iter()
                .map(|record| (&record.location.database, &record.location.schema)),
        ),
    )?;
    let table = batch.kind.mirror_table();
    let source_table = batch.kind.source_table().ok_or_else(|| {
        ControlError::Invalid(format!(
            "{} is not a state-table history",
            batch.kind.as_str()
        ))
    })?;
    let mut transaction = begin_ingestion(
        store,
        (location_id, target),
        (batch.session_id, batch.generation),
    )
    .await?;
    let mut applied = false;
    let mut keys: Vec<Uuid> = Vec::new();
    for (record, delivered) in records.into_iter().zip(&batch.records) {
        let source_id = source_id(batch.kind, delivered)?.ok_or_else(|| {
            ControlError::Invalid(format!("{} has no source key", batch.kind.as_str()))
        })?;
        let event_id = scoped_event_id(
            (location_id, target),
            (
                record.location.database.as_deref(),
                &record.location.schema,
                source_table,
            ),
            &source_id,
        )?;
        keys.push(event_id);
        let mut columns = vec![
            "event_id",
            "location_id",
            "target",
            "observed_at",
            "state_database",
            "state_schema",
        ];
        let mut parameters = vec![
            Parameter::Id(event_id),
            Parameter::Id(location_id),
            Parameter::Text(target.into()),
            Parameter::Time(record.observed_at),
            Parameter::OptionalText(record.location.database),
            Parameter::Text(record.location.schema),
        ];
        for (column, value) in record.columns {
            columns.push(column);
            parameters.push(value);
        }
        columns.extend(["record_json", "received_at"]);
        parameters.extend([
            Parameter::Json(record.record_json),
            Parameter::Time(transaction.now),
        ]);
        let placeholders = Parameter::placeholders(1..=parameters.len());
        let sql = format!(
            "INSERT INTO {table} ({}) VALUES ({placeholders}) ON CONFLICT (event_id) DO NOTHING RETURNING event_id",
            columns.join(",")
        );
        let inserted;
        (transaction, inserted) = query(transaction, &sql, parameters).await?;
        applied |= !inserted.is_empty();
    }
    commit_ingestion(
        transaction,
        (table, location_id, target),
        (batch.scan.as_ref(), &keys),
        applied,
    )
    .await
}

#[derive(Deserialize)]
struct EventKey {
    event_id: String,
}

#[derive(Deserialize)]
struct AuditResultKey {
    result_id: String,
}

#[derive(Deserialize)]
struct ObservationKey {
    observation_id: String,
}

/// Node outcomes carry no event identity; their identity columns hash to one.
#[derive(Deserialize)]
struct NodeResultKey {
    node_type: Option<String>,
    node_name: Option<String>,
    target_database: Option<String>,
    target_schema: Option<String>,
    target_name: Option<String>,
    run_id: Option<String>,
    status: Option<String>,
    ts: DateTime<Utc>,
}

/// Consumed input versions carry no event identity; their identity columns hash to one.
#[derive(Deserialize)]
struct ConsumptionKey {
    execution_id: Option<String>,
    phase: Option<String>,
    input_kind: Option<String>,
    input_name: Option<String>,
    input_version: Option<String>,
    recorded_at: DateTime<Utc>,
}

fn key<T: DeserializeOwned>(kind: WarehouseHistoryKind, record: &Value) -> Result<T, ControlError> {
    T::deserialize(record)
        .map_err(|error| ControlError::Invalid(format!("invalid {} key: {error}", kind.as_str())))
}

/// The source id of a full record or its key projection; `None` for kinds that read whole rows.
pub(crate) fn source_id(
    kind: WarehouseHistoryKind,
    record: &Value,
) -> Result<Option<String>, ControlError> {
    Ok(Some(match kind {
        WarehouseHistoryKind::JanitorEvents
        | WarehouseHistoryKind::Migrations
        | WarehouseHistoryKind::OldNameViews
        | WarehouseHistoryKind::Microbatches
        | WarehouseHistoryKind::ColumnMigrations => key::<EventKey>(kind, record)?.event_id,
        WarehouseHistoryKind::AuditResults => key::<AuditResultKey>(kind, record)?.result_id,
        WarehouseHistoryKind::Observations => key::<ObservationKey>(kind, record)?.observation_id,
        WarehouseHistoryKind::NodeResults => {
            let key: NodeResultKey = key(kind, record)?;
            sha256_hex(
                json!([
                    key.node_type,
                    key.node_name,
                    key.target_database,
                    key.target_schema,
                    key.target_name,
                    key.run_id,
                    key.status,
                    key.ts,
                ])
                .to_string()
                .as_bytes(),
            )
        }
        WarehouseHistoryKind::Consumption => {
            let key: ConsumptionKey = key(kind, record)?;
            sha256_hex(
                json!([
                    key.execution_id,
                    key.phase,
                    key.input_kind,
                    key.input_name,
                    key.input_version,
                    key.recorded_at,
                ])
                .to_string()
                .as_bytes(),
            )
        }
        WarehouseHistoryKind::SourceFreshness
        | WarehouseHistoryKind::Fingerprints
        | WarehouseHistoryKind::Catalog
        | WarehouseHistoryKind::Statistics => return Ok(None),
    }))
}

#[derive(Deserialize)]
struct Held {
    event_id: Uuid,
}

/// The mirror ids of a key page's rows, in page order.
pub(crate) fn key_page_ids(
    (location_id, target): (Uuid, &str),
    page: &MetadataKeyPage,
) -> Result<Vec<Uuid>, ControlError> {
    let mut ids: Vec<Uuid> = Vec::with_capacity(page.keys.len());
    for key in &page.keys {
        let (Some(source_table), Some(source_id)) =
            (page.kind.source_table(), source_id(page.kind, key)?)
        else {
            return Err(ControlError::Invalid(format!(
                "{} has no key pages",
                page.kind.as_str().replace('_', " ")
            )));
        };
        ids.push(scoped_event_id(
            (location_id, target),
            (
                page.scan.location.database.as_deref(),
                &page.scan.location.schema,
                source_table,
            ),
            &source_id,
        )?);
    }
    Ok(ids)
}

/// Mark the mirror rows a scan saw by key; returns the ids the mirror holds.
pub(crate) async fn tag_held(
    transaction: ControlTransaction,
    (table, scan_id): (&str, Uuid),
    ids: &[Uuid],
) -> Result<(ControlTransaction, HashSet<Uuid>), ControlError> {
    if ids.is_empty() {
        return Ok((transaction, HashSet::new()));
    }
    let mut parameters = vec![Parameter::Id(scan_id)];
    parameters.extend(ids.iter().copied().map(Parameter::Id));
    let placeholders = Parameter::placeholders(2..=parameters.len());
    let (transaction, rows): (ControlTransaction, Vec<Held>) = query_records(
        transaction,
        &format!("UPDATE {table} SET scan_id=$1 WHERE event_id IN ({placeholders}) RETURNING event_id"),
        parameters,
    )
    .await?;
    Ok((transaction, rows.into_iter().map(|row| row.event_id).collect()))
}
