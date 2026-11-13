//! Mirror reads share database scoping and the keyset cursor; previews read each node's newest facts.

use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Utc};
use serde::de::DeserializeOwned;
use serde_json::Value;
use uuid::Uuid;

use crate::platform::models::{LocationHealth, LocationHealthObservation};
use crate::store::errors::ControlError;
use crate::store::main::decompress::decompress;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};
use crate::store::types::Row;
use crate::warehouse_metadata::models::{
    PreviewLatestRun, WarehouseCatalogRecord, WarehouseConsumptionRecord,
    WarehouseFingerprintRecord, WarehouseHistoryKind, WarehouseObservationRecord,
};

const RELATIONS_SQL: &str = "SELECT record_json FROM sqb_warehouse_catalog \
    WHERE location_id=$1 AND target=$2 ORDER BY state_database, state_schema, event_id";
const FINGERPRINTS_SQL: &str = "SELECT f.record_json, t.body FROM (SELECT record_json, definition_text_hash, \
    ROW_NUMBER() OVER (PARTITION BY state_database, state_schema, node_type, node_name, target_database, \
    target_schema, target_name ORDER BY observed_at DESC, event_id DESC) AS newest \
    FROM sqb_warehouse_fingerprints WHERE location_id=$1 AND target=$2) f \
    JOIN sqb_source_texts t ON t.text_hash=f.definition_text_hash WHERE f.newest=1 \
    ORDER BY f.record_json ->> 'node_type', f.record_json ->> 'node_name'";
const LATEST_RUNS_SQL: &str = "SELECT record_json FROM (SELECT record_json, ROW_NUMBER() OVER (PARTITION BY \
    state_database, state_schema, record_json ->> 'node_type', record_json ->> 'node_name' \
    ORDER BY observed_at DESC, event_id DESC) AS newest FROM sqb_warehouse_consumption \
    WHERE location_id=$1 AND target=$2 AND record_json ->> 'phase'='succeeded' \
    AND record_json ->> 'run_id' IS NOT NULL) runs WHERE newest=1 \
    ORDER BY record_json ->> 'node_type', record_json ->> 'node_name'";
const OBSERVATIONS_SQL: &str = "SELECT record_json FROM (SELECT record_json, kind, node, ROW_NUMBER() OVER \
    (PARTITION BY state_database, state_schema, kind, node ORDER BY observed_at DESC, event_id DESC) AS newest \
    FROM sqb_warehouse_observations WHERE location_id=$1 AND target=$2) observations WHERE newest=1 \
    ORDER BY kind, node";

/// Adds scope, cursor and page size to the parameters, with the SQL after the caller's conditions.
pub(crate) fn page_tail(
    mut parameters: Vec<Parameter>,
    database: Option<&String>,
    after: (Option<DateTime<Utc>>, Option<Uuid>),
    limit: usize,
) -> (Vec<Parameter>, String) {
    let database = database.map_or("state_database IS NULL".to_owned(), |database| {
        parameters.push(Parameter::Text(database.clone()));
        format!("state_database=${}", parameters.len())
    });
    let after = after
        .0
        .zip(after.1)
        .map_or(String::new(), |(observed_at, event_id)| {
            parameters.extend([Parameter::Time(observed_at), Parameter::Id(event_id)]);
            let position = parameters.len();
            format!(
                " AND (observed_at,event_id)>(${},${position})",
                position - 1
            )
        });
    parameters.push(Parameter::Integer(limit as i64));
    let tail = format!(
        " AND {database}{after} ORDER BY observed_at,event_id LIMIT ${}",
        parameters.len()
    );
    (parameters, tail)
}

fn scope_parameters((location_id, target): (Uuid, &str)) -> Vec<Parameter> {
    vec![Parameter::Id(location_id), Parameter::Text(target.into())]
}

fn decoded<T: DeserializeOwned>(mut row: Row) -> Result<T, ControlError> {
    serde_json::from_value(row.remove("record_json").unwrap_or(Value::Null))
        .map_err(|error| ControlError::Invalid(format!("invalid mirrored record: {error}")))
}

async fn mirrored<T: DeserializeOwned>(
    transaction: ControlTransaction,
    sql: &str,
    scope: (Uuid, &str),
) -> Result<(ControlTransaction, Vec<T>), ControlError> {
    let (transaction, rows) = query(transaction, sql, scope_parameters(scope)).await?;
    let mut records: Vec<T> = Vec::with_capacity(rows.len());
    for row in rows {
        records.push(decoded(row)?);
    }
    Ok((transaction, records))
}

/// Every relation of the target's newest complete catalog scans.
pub(crate) async fn preview_relations(
    transaction: ControlTransaction,
    scope: (Uuid, &str),
) -> Result<(ControlTransaction, Vec<WarehouseCatalogRecord>), ControlError> {
    mirrored(transaction, RELATIONS_SQL, scope).await
}

/// Each node's newest fingerprint, with its definition restored from the shared text store.
pub(crate) async fn preview_fingerprints(
    transaction: ControlTransaction,
    scope: (Uuid, &str),
) -> Result<(ControlTransaction, Vec<WarehouseFingerprintRecord>), ControlError> {
    let (transaction, rows) = query(transaction, FINGERPRINTS_SQL, scope_parameters(scope)).await?;
    let mut fingerprints: Vec<WarehouseFingerprintRecord> = Vec::with_capacity(rows.len());
    for mut row in rows {
        let body: Vec<u8> = decompress(&row.remove("body").unwrap_or(Value::Null))?;
        let definition: String =
            String::from_utf8(body).map_err(|error| ControlError::Invalid(error.to_string()))?;
        let mut record: Value = row.remove("record_json").unwrap_or(Value::Null);
        if let Value::Object(object) = &mut record {
            object.insert("definition".into(), Value::String(definition));
        }
        fingerprints.push(serde_json::from_value(record).map_err(|error| {
            ControlError::Invalid(format!("invalid mirrored fingerprint: {error}"))
        })?);
    }
    Ok((transaction, fingerprints))
}

/// Each node's newest succeeded build that names its node and run.
pub(crate) async fn preview_latest_runs(
    transaction: ControlTransaction,
    scope: (Uuid, &str),
) -> Result<(ControlTransaction, Vec<PreviewLatestRun>), ControlError> {
    let (transaction, consumption): (ControlTransaction, Vec<WarehouseConsumptionRecord>) =
        mirrored(transaction, LATEST_RUNS_SQL, scope).await?;
    let mut runs: Vec<PreviewLatestRun> = Vec::with_capacity(consumption.len());
    for record in consumption {
        if let (Some(node_type), Some(node_name), Some(run_id)) =
            (record.node_type, record.node_name, record.run_id)
        {
            runs.push(PreviewLatestRun {
                state_database: record.state_database,
                state_schema: record.state_schema,
                node_type,
                node_name,
                run_id,
            });
        }
    }
    Ok((transaction, runs))
}

/// Each node's newest observation of each kind.
pub(crate) async fn preview_observations(
    transaction: ControlTransaction,
    scope: (Uuid, &str),
) -> Result<(ControlTransaction, Vec<WarehouseObservationRecord>), ControlError> {
    mirrored(transaction, OBSERVATIONS_SQL, scope).await
}

/// The oldest of each required table's newest clean sync; `None` until every table synced cleanly.
pub(crate) fn synced_at(
    health: &[LocationHealthObservation],
    tables: &[WarehouseHistoryKind],
) -> Option<DateTime<Utc>> {
    let mut newest: BTreeMap<(Option<&str>, &str, &str), DateTime<Utc>> = BTreeMap::new();
    for observation in health {
        let LocationHealth::WarehouseSync {
            location,
            table,
            failure: None,
            ..
        } = &observation.report.health
        else {
            continue;
        };
        if !tables.contains(table) {
            continue;
        }
        let key = (
            location.database.as_deref(),
            location.schema.as_str(),
            table.as_str(),
        );
        let at = newest.entry(key).or_insert(observation.received_at);
        *at = (*at).max(observation.received_at);
    }
    let synced: BTreeSet<&str> = newest.keys().map(|(_, _, table)| *table).collect();
    let complete: bool = tables.iter().all(|table| synced.contains(table.as_str()));
    complete.then(|| newest.values().min().copied()).flatten()
}
