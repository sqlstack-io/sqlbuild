//! One fenced, atomic persistence path for immutable warehouse events with typed filter columns.

use uuid::Uuid;

use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlStore, Parameter};
use crate::warehouse_metadata::_helpers::ingestion::{
    begin_ingestion, commit_ingestion, scoped_event_id, validate_fields,
};
use crate::warehouse_metadata::_helpers::reconciliation::begin_reconciliation;
use crate::warehouse_metadata::models::{StateEventsImport, WarehouseSyncResponse};

pub(crate) async fn ingest_events(
    store: &ControlStore,
    (location_id, target): (Uuid, &str),
    batch: StateEventsImport<'_>,
) -> Result<WarehouseSyncResponse, ControlError> {
    let label = batch.kind.as_str().replace('_', " ");
    validate_fields(
        (&label, batch.generation, batch.records.len()),
        batch.invalid_identity,
        batch.reconciliation.is_some_and(|page| {
            batch.records.iter().any(|record| {
                record.location.database != page.state_database
                    || record.location.schema != page.state_schema
                    || page
                        .day
                        .is_some_and(|day| record.observed_at.date_naive() != day)
            })
        }),
    )?;
    let table = batch.kind.mirror_table();
    let source_table = format!("_sqlbuild_{}", batch.kind.as_str());
    let mut transaction = begin_ingestion(
        store,
        (location_id, target),
        (batch.session_id, batch.generation),
    )
    .await?;
    if let Some(page) = batch.reconciliation {
        transaction = begin_reconciliation(
            transaction,
            (table, location_id, target),
            page,
            batch.payload,
        )
        .await?;
    }
    let mut applied = false;
    let mut keys: Vec<Uuid> = Vec::new();
    for record in batch.records {
        let event_id = scoped_event_id(
            (location_id, target),
            (
                record.location.database.as_deref(),
                &record.location.schema,
                &source_table,
            ),
            &record.source_id,
        )?;
        if batch.reconciliation.is_some() {
            keys.push(event_id);
        }
        let mut columns = vec![
            "event_id",
            "location_id",
            "target",
            "source_event_id",
            "observed_at",
            "state_database",
            "state_schema",
        ];
        let mut parameters = vec![
            Parameter::Id(event_id),
            Parameter::Id(location_id),
            Parameter::Text(target.into()),
            Parameter::Text(record.source_id),
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
        let placeholders = (1..=parameters.len())
            .map(|index| format!("${index}"))
            .collect::<Vec<_>>()
            .join(",");
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
        (batch.reconciliation, &keys),
        applied,
    )
    .await
}
