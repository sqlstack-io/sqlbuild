//! Mirror reads share database scoping and the keyset cursor; previews read each node's newest facts.

use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Utc};
use serde::de::DeserializeOwned;
use serde_json::Value;
use uuid::Uuid;

use crate::authorization::models::CurrentSnapshotId;
use crate::platform::constants::LOCATION_RETENTION_DAYS;
use crate::snapshots::main::read_snapshot_nodes::read_snapshot_nodes;
use crate::store::errors::ControlError;
use crate::store::main::decode_row::decode_row;
use crate::store::main::decompress::decompress;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};
use crate::store::types::Row;
use crate::warehouse_metadata::constants::PREVIEW_REQUIRED_TABLES;
use crate::warehouse_metadata::models::{
    CleanSync, PreviewLatestRun, WarehouseCatalogRecord, WarehouseConsumptionRecord,
    WarehouseFingerprintRecord, WarehouseObservationRecord,
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
/// Every consumption row the warehouse still keeps, which complete scans bound.
const CONSUMPTION_SQL: &str = "SELECT record_json FROM sqb_warehouse_consumption \
    WHERE location_id=$1 AND target=$2 ORDER BY observed_at, event_id";
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

/// Every mirrored consumption row: a build reads the newest executions and any older one by ID.
pub(crate) async fn preview_consumption(
    transaction: ControlTransaction,
    scope: (Uuid, &str),
) -> Result<(ControlTransaction, Vec<WarehouseConsumptionRecord>), ControlError> {
    mirrored(transaction, CONSUMPTION_SQL, scope).await
}

/// Each node's newest observation of each kind.
pub(crate) async fn preview_observations(
    transaction: ControlTransaction,
    scope: (Uuid, &str),
) -> Result<(ControlTransaction, Vec<WarehouseObservationRecord>), ControlError> {
    mirrored(transaction, OBSERVATIONS_SQL, scope).await
}

/// When every schema the current snapshot builds into last synced each required table cleanly.
pub(crate) async fn preview_synced_at(
    transaction: ControlTransaction,
    (location_id, target): (Uuid, &str),
) -> Result<(ControlTransaction, Option<DateTime<Utc>>), ControlError> {
    let (transaction, current) = query(
        transaction,
        "SELECT snapshot_id FROM sqb_current_snapshots WHERE location_id=$1 AND target=$2",
        vec![Parameter::Id(location_id), Parameter::Text(target.into())],
    )
    .await?;
    let current: Option<CurrentSnapshotId> =
        current.into_iter().next().map(decode_row).transpose()?;
    let Some(current) = current else {
        return Ok((transaction, None));
    };
    let (transaction, nodes) = read_snapshot_nodes(transaction, current.snapshot_id, None).await?;
    let schemas: BTreeSet<(Option<String>, String)> = nodes
        .values()
        .filter_map(|node| {
            let destination = &node.definition["destination"];
            Some((
                destination["database"].as_str().map(str::to_owned),
                destination["schema"].as_str()?.to_owned(),
            ))
        })
        .collect();
    let today = transaction.now.date_naive();
    let days = 0..=LOCATION_RETENTION_DAYS;
    let invocations: Vec<Parameter> = days
        .map(|day| {
            Parameter::Text(format!(
                "location/{location_id}/{}",
                today - chrono::TimeDelta::days(day)
            ))
        })
        .collect();
    let target_placeholder = invocations.len() + 1;
    let placeholders = Parameter::placeholders(1..=invocations.len());
    let mut parameters = invocations;
    parameters.push(Parameter::Text(target.into()));
    let (transaction, rows) = query(
        transaction,
        &format!(
            "SELECT payload->'report'->'health'->'location'->>'database' AS state_database, \
             payload->'report'->'health'->'location'->>'schema' AS state_schema, \
             payload->'report'->'health'->>'table' AS state_table, MAX(occurred_at) AS synced_at \
             FROM recall_events WHERE invocation_id IN ({placeholders}) AND event_type='location_health' \
             AND payload->'report'->>'target'=${target_placeholder} \
             AND payload->'report'->'health'->>'kind'='warehouse_sync' \
             AND payload->'report'->'health'->>'failure' IS NULL GROUP BY 1, 2, 3"
        ),
        parameters,
    )
    .await?;
    let newest: BTreeMap<(Option<String>, String, String), DateTime<Utc>> = rows
        .into_iter()
        .map(|row| decode_row::<CleanSync>(row).map(clean_sync_entry))
        .collect::<Result<_, _>>()?;
    let mut oldest: Option<DateTime<Utc>> = None;
    for (database, schema) in &schemas {
        for table in PREVIEW_REQUIRED_TABLES {
            let Some(at) = newest.get(&(database.clone(), schema.clone(), table.as_str().into()))
            else {
                return Ok((transaction, None));
            };
            oldest = Some(oldest.map_or(*at, |oldest| oldest.min(*at)));
        }
    }
    Ok((transaction, oldest))
}

fn clean_sync_entry(sync: CleanSync) -> ((Option<String>, String, String), DateTime<Utc>) {
    (
        (sync.state_database, sync.state_schema, sync.state_table),
        sync.synced_at,
    )
}
