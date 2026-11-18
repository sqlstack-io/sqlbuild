//! A metadata page shares one registered session, snapshot fence and physical identity scope.

use serde::de::DeserializeOwned;
use uuid::Uuid;

use crate::platform::main::installation::require_instance::require_instance;
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{Change, ControlStore, ControlTransaction, Parameter, TransactionMode};
use crate::warehouse_metadata::constants::{
    IGNORED_KEY, MAX_HISTORY_PAGE_RECORDS, MAX_HISTORY_RECORDS,
};
use crate::warehouse_metadata::models::{
    MetadataBatch, MetadataScan, WarehouseHistoryKind, WarehouseSyncResponse,
};

pub(crate) async fn begin_ingestion(
    store: &ControlStore,
    (location_id, target): (Uuid, &str),
    (session_id, generation): (Uuid, i64),
) -> Result<ControlTransaction, ControlError> {
    let transaction = require_instance(
        begin(store, TransactionMode::Write).await?,
        (location_id, session_id, target),
    )
    .await?;
    let (transaction, rows) = query(
        transaction,
        "SELECT s.snapshot_id FROM sqb_current_snapshots c JOIN sqb_snapshots s ON s.snapshot_id=c.snapshot_id \
         WHERE c.location_id=$1 AND c.target=$2 AND s.generation=$3",
        vec![Parameter::Id(location_id), Parameter::Text(target.into()), Parameter::Integer(generation)],
    ).await?;
    if rows.is_empty() {
        return Err(ControlError::Fenced);
    }
    Ok(transaction)
}

pub(crate) fn batch_records<T: DeserializeOwned>(
    batch: &MetadataBatch,
) -> Result<Vec<T>, ControlError> {
    batch
        .records
        .iter()
        .map(|record| serde_json::from_value(record.clone()))
        .collect::<Result<_, _>>()
        .map_err(|error| {
            ControlError::Invalid(format!("invalid {} record: {error}", batch.kind.as_str()))
        })
}

pub(crate) fn scoped_event_id(
    (location_id, target): (Uuid, &str),
    (database, schema, table): (Option<&str>, &str, &str),
    source_event_id: &str,
) -> Result<Uuid, ControlError> {
    let identity = serde_json::to_vec(&(target, database, schema, table, source_event_id))
        .map_err(|error| ControlError::Invalid(error.to_string()))?;
    Ok(Uuid::new_v5(&location_id, &identity))
}

/// Commit the records a page applied so far and open its next transaction behind the same fence.
pub(crate) async fn next_ingestion_chunk(
    mut transaction: ControlTransaction,
    (store, location_id, target): (&ControlStore, Uuid, &str),
    batch: &MetadataBatch,
    applied: bool,
) -> Result<ControlTransaction, ControlError> {
    if applied {
        transaction
            .changes
            .insert(Change::NodeStatus(location_id, target.into()));
    }
    commit(transaction).await?;
    begin_ingestion(
        store,
        (location_id, target),
        (batch.session_id, batch.generation),
    )
    .await
}

/// Delivered rows take the scan's id; a complete scan deletes every row it did not deliver.
pub(crate) async fn commit_ingestion(
    mut transaction: ControlTransaction,
    (table, location_id, target): (&str, Uuid, &str),
    (scan, keys): (Option<&MetadataScan>, &[Uuid]),
    mut applied: bool,
) -> Result<WarehouseSyncResponse, ControlError> {
    if let Some(scan) = scan {
        if !keys.is_empty() {
            let mut parameters = vec![Parameter::Id(scan.scan_id)];
            parameters.extend(keys.iter().copied().map(Parameter::Id));
            let placeholders = Parameter::placeholders(2..=parameters.len());
            (transaction, _) = query(
                transaction,
                &format!("UPDATE {table} SET scan_id=$1 WHERE event_id IN ({placeholders})"),
                parameters,
            )
            .await?;
        }
        if scan.complete {
            let mut parameters = vec![
                Parameter::Id(location_id),
                Parameter::Text(target.into()),
                Parameter::Text(scan.location.schema.clone()),
                Parameter::Id(scan.scan_id),
            ];
            let database = match &scan.location.database {
                Some(database) => {
                    parameters.push(Parameter::Text(database.clone()));
                    "state_database=$5"
                }
                None => "state_database IS NULL",
            };
            let removed;
            (transaction, removed) = query(
                transaction,
                &format!(
                    "DELETE FROM {table} WHERE location_id=$1 AND target=$2 AND state_schema=$3 \
                     AND {database} AND (scan_id IS NULL OR scan_id<>$4) RETURNING event_id"
                ),
                parameters,
            )
            .await?;
            applied |= !removed.is_empty();
        }
    }
    if applied {
        transaction
            .changes
            .insert(Change::NodeStatus(location_id, target.into()));
    }
    commit(transaction).await?;
    Ok(WarehouseSyncResponse { applied })
}

/// Every record of a scan page must come from the scanned physical namespace.
pub(crate) fn outside_scan<'a>(
    batch: &MetadataBatch,
    mut locations: impl Iterator<Item = (&'a Option<String>, &'a String)>,
) -> bool {
    batch.scan.as_ref().is_some_and(|scan| {
        locations.any(|(database, schema)| {
            *database != scan.location.database || *schema != scan.location.schema
        })
    })
}

pub(crate) fn validate_fields(
    batch: &MetadataBatch,
    invalid_identity: bool,
    invalid_scope: bool,
) -> Result<(), ControlError> {
    let kind = batch.kind.as_str().replace('_', " ");
    let most = if batch.kind == WarehouseHistoryKind::Statistics {
        MAX_HISTORY_RECORDS
    } else {
        MAX_HISTORY_PAGE_RECORDS
    };
    if batch.generation < 1 || batch.records.len() > most || invalid_identity {
        return Err(ControlError::Invalid(format!(
            "{kind} sync needs a positive generation, at most {most} records and nonempty state identities"
        )));
    }
    if invalid_scope
        || batch
            .scan
            .as_ref()
            .is_some_and(|scan| scan.location.schema.trim().is_empty())
    {
        return Err(ControlError::Invalid(format!(
            "{kind} scan records must belong to its physical namespace"
        )));
    }
    Ok(())
}

/// Whether an audit result's metadata marks it as a failure its build ignored.
pub(crate) fn ignored_failure(metadata_json: Option<&str>) -> bool {
    let Some(text) = metadata_json else {
        return false;
    };
    match serde_json::from_str::<serde_json::Value>(text) {
        Ok(metadata) => metadata
            .get(IGNORED_KEY)
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
        Err(error) if error.is_syntax() || error.is_eof() => false,
        Err(_) => false,
    }
}
