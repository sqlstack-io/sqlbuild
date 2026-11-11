//! One fenced, atomic persistence path for immutable warehouse events with typed filter columns.

use uuid::Uuid;

use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlStore, Parameter};
use crate::warehouse_metadata::_helpers::ingestion::{
    begin_ingestion, commit_ingestion, outside_scan, scoped_event_id, validate_fields,
};
use crate::warehouse_metadata::models::{MetadataBatch, StateEventImport, WarehouseSyncResponse};

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
    for record in records {
        let event_id = scoped_event_id(
            (location_id, target),
            (
                record.location.database.as_deref(),
                &record.location.schema,
                source_table,
            ),
            &record.source_id,
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
