//! Node conditions consume executed run-node outcomes or the latest warehouse source observations.

use std::collections::BTreeSet;
use std::num::NonZeroU32;

use chrono::{DateTime, NaiveDateTime, TimeDelta, Utc};
use sqlbuild_recall::records::main::sha256_hex::sha256_hex;
use uuid::Uuid;

use crate::sensors::_helpers::requests::queue_sensor_run;
use crate::sensors::_helpers::transitions::matches_transition;
use crate::sensors::constants::TIMESTAMP_VALUE_KIND;
use crate::sensors::models::{
    NodeEventKind, NodeSensorEvent, SensorCursor, SensorEvaluationRequest,
};
use crate::sensors::types::SourcePage;
use crate::snapshots::main::read_snapshot_nodes::read_snapshot_nodes;
use crate::store::errors::ControlError;
use crate::store::main::decode_row::decode_row;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};
use crate::warehouse_metadata::models::WarehouseFreshnessRecord;

pub(crate) async fn consume_node_events(
    transaction: ControlTransaction,
    evaluation: &SensorEvaluationRequest,
    selection: &BTreeSet<String>,
    kinds: &BTreeSet<NodeEventKind>,
) -> Result<(ControlTransaction, (SensorCursor, Vec<Uuid>)), ControlError> {
    let (mut transaction, rows) = query(
        transaction,
        "SELECT n.run_id,n.node_key,n.status,n.finished_storage_id FROM sqb_run_nodes n \
         JOIN sqb_runs r ON r.run_id=n.run_id \
         WHERE n.finished_storage_id>$1 AND n.status IN ('success','failed') \
         AND n.started_at IS NOT NULL AND r.location_id=$2 AND r.target=$3 AND r.kind<>'group' \
         ORDER BY n.finished_storage_id LIMIT 100",
        vec![
            Parameter::Integer(evaluation.cursor.event_storage_id),
            Parameter::Id(evaluation.location_id),
            Parameter::Text(evaluation.target.clone()),
        ],
    )
    .await?;
    let mut cursor = evaluation.cursor.clone();
    let mut run_ids: Vec<Uuid> = Vec::new();
    for row in rows {
        let node: NodeSensorEvent = decode_row(row)?;
        cursor.event_storage_id = node.finished_storage_id;
        if !selection.contains(&node.node_key) {
            continue;
        }
        let matches;
        (transaction, matches) = matches_transition(transaction, evaluation, &node, kinds).await?;
        if matches {
            let run_id;
            (transaction, run_id) = queue_sensor_run(
                transaction,
                evaluation,
                &format!("{}/{}", node.run_id, node.node_key),
            )
            .await?;
            run_ids.push(run_id);
        }
    }
    Ok((transaction, (cursor, run_ids)))
}

/// A source's newest received version queues one run per change from the version received before it.
pub(crate) async fn consume_source_events(
    transaction: ControlTransaction,
    evaluation: &SensorEvaluationRequest,
    selection: &BTreeSet<String>,
) -> Result<(ControlTransaction, (SensorCursor, Vec<Uuid>)), ControlError> {
    let (mut transaction, (nodes, next)) = source_page(transaction, evaluation, selection).await?;
    let mut run_ids: Vec<Uuid> = Vec::new();
    for (key, definition) in nodes {
        let (parameters, predicate) = source_scope(evaluation, &key, &definition);
        let rows;
        (transaction, rows) = query(transaction, &format!(
            "SELECT latest.record_json, (SELECT earlier.event_id FROM sqb_warehouse_freshness earlier \
             WHERE earlier.location_id=$1 AND earlier.target=$2 AND earlier.record_json ->> 'source_name'=$3{earlier} \
             AND earlier.record_json ->> 'data_version' IS NOT NULL \
             AND earlier.record_json ->> 'data_version_hash'<>latest.record_json ->> 'data_version_hash' \
             AND (earlier.received_at,earlier.observed_at,earlier.event_id)<(latest.received_at,latest.observed_at,latest.event_id) \
             ORDER BY earlier.received_at DESC,earlier.observed_at DESC,earlier.event_id DESC LIMIT 1) AS changed_from \
             FROM sqb_warehouse_freshness latest WHERE latest.location_id=$1 AND latest.target=$2 \
             AND latest.record_json ->> 'source_name'=$3{latest} AND latest.record_json ->> 'data_version' IS NOT NULL \
             ORDER BY latest.received_at DESC,latest.observed_at DESC,latest.event_id DESC LIMIT 1",
            earlier = predicate.replace("record_json", "earlier.record_json"),
            latest = predicate.replace("record_json", "latest.record_json"),
        ), parameters).await?;
        let Some(row) = rows.first() else { continue };
        let record: WarehouseFreshnessRecord =
            serde_json::from_value(row["record_json"].clone())
                .map_err(|error| ControlError::Invalid(error.to_string()))?;
        let identity = serde_json::to_vec(&(
            &key,
            &record.target_database,
            &record.target_schema,
            &record.target_name,
            &record.data_version_hash,
            &row["changed_from"],
        ))
        .map_err(|error| ControlError::Invalid(error.to_string()))?;
        let run_key = sha256_hex(&identity);
        let requested;
        (transaction, requested) = query(
            transaction,
            "SELECT 1 AS requested FROM sqb_run_requests WHERE trigger_id=$1 AND run_key=$2",
            vec![
                Parameter::Id(evaluation.trigger_id),
                Parameter::Text(run_key.clone()),
            ],
        )
        .await?;
        if !requested.is_empty() {
            continue;
        }
        let run_id;
        (transaction, run_id) = queue_sensor_run(transaction, evaluation, &run_key).await?;
        run_ids.push(run_id);
    }
    let mut cursor = evaluation.cursor.clone();
    cursor.custom = next;
    Ok((transaction, (cursor, run_ids)))
}

/// One page of selected source definitions after the cursor's last key, and the next page's start.
async fn source_page(
    transaction: ControlTransaction,
    evaluation: &SensorEvaluationRequest,
    selection: &BTreeSet<String>,
) -> Result<SourcePage, ControlError> {
    let mut remaining = selection.iter().filter(|key| {
        evaluation
            .cursor
            .custom
            .as_ref()
            .is_none_or(|after| *key > after)
    });
    let page: BTreeSet<String> = remaining.by_ref().take(100).cloned().collect();
    let next = remaining
        .next()
        .is_some()
        .then(|| page.last().cloned())
        .flatten();
    let (transaction, nodes) =
        read_snapshot_nodes(transaction, evaluation.snapshot_id, Some(&page)).await?;
    Ok((
        transaction,
        (
            nodes
                .into_iter()
                .map(|(key, node)| (key, node.definition))
                .collect(),
            next,
        ),
    ))
}

/// Parameters $1-$3 and a predicate matching the declared source's physical relation.
fn source_scope(
    evaluation: &SensorEvaluationRequest,
    key: &str,
    definition: &serde_json::Value,
) -> (Vec<Parameter>, String) {
    let mut parameters = vec![
        Parameter::Id(evaluation.location_id),
        Parameter::Text(evaluation.target.clone()),
        Parameter::Text(key.replacen("source:", "", 1)),
    ];
    let mut predicate = String::new();
    for (column, value) in [
        (
            "target_database",
            definition["namespace"]
                .get(0)
                .unwrap_or(&definition["database"]),
        ),
        (
            "target_schema",
            definition["namespace"]
                .get(1)
                .unwrap_or(&definition["schema"]),
        ),
        ("target_name", &definition["table"]),
    ] {
        if let Some(value) = value.as_str() {
            parameters.push(Parameter::Text(value.into()));
            predicate.push_str(&format!(
                " AND record_json ->> '{column}'=${}",
                parameters.len()
            ));
        } else {
            predicate.push_str(&format!(" AND record_json ->> '{column}' IS NULL"));
        }
    }
    (parameters, predicate)
}

fn stale_version(
    record: &WarehouseFreshnessRecord,
    now: DateTime<Utc>,
    after_seconds: NonZeroU32,
) -> Result<bool, ControlError> {
    let Some(version) = record
        .data_version
        .as_deref()
        .filter(|_| record.value_kind == TIMESTAMP_VALUE_KIND)
    else {
        return Ok(false);
    };
    let instant = DateTime::parse_from_rfc3339(version)
        .map(|value| value.with_timezone(&Utc))
        .or_else(|_| {
            NaiveDateTime::parse_from_str(version, "%Y-%m-%dT%H:%M:%S%.f")
                .map(|value| value.and_utc())
        })
        .map_err(|_| {
            ControlError::Invalid(
                "timestamp source freshness version is not an ISO datetime".into(),
            )
        })?;
    Ok(now.signed_duration_since(instant) > TimeDelta::seconds(i64::from(after_seconds.get())))
}

/// Reevaluate current timestamp age; only page position and idempotent requests are stored.
pub(crate) async fn consume_stale_sources(
    transaction: ControlTransaction,
    evaluation: &SensorEvaluationRequest,
    selection: &BTreeSet<String>,
    after_seconds: NonZeroU32,
) -> Result<(ControlTransaction, (SensorCursor, Vec<Uuid>)), ControlError> {
    let (mut transaction, (nodes, next)) = source_page(transaction, evaluation, selection).await?;
    let mut run_ids: Vec<Uuid> = Vec::new();
    for (key, definition) in nodes {
        let (parameters, predicate) = source_scope(evaluation, &key, &definition);
        let rows;
        (transaction, rows) = query(transaction, &format!(
            "SELECT record_json FROM sqb_warehouse_freshness WHERE location_id=$1 AND target=$2 \
             AND record_json ->> 'source_name'=$3{predicate} ORDER BY observed_at DESC,event_id DESC LIMIT 1"
        ), parameters).await?;
        let Some(row) = rows.first() else { continue };
        let record: WarehouseFreshnessRecord =
            serde_json::from_value(row["record_json"].clone())
                .map_err(|error| ControlError::Invalid(error.to_string()))?;
        if !stale_version(&record, evaluation.requested_at, after_seconds)? {
            continue;
        }
        let identity = serde_json::to_vec(&(
            &key,
            &record.target_database,
            &record.target_schema,
            &record.target_name,
            &record.data_version_hash,
            after_seconds,
        ))
        .map_err(|error| ControlError::Invalid(error.to_string()))?;
        let run_id;
        (transaction, run_id) =
            queue_sensor_run(transaction, evaluation, &sha256_hex(&identity)).await?;
        run_ids.push(run_id);
    }
    let mut cursor = evaluation.cursor.clone();
    cursor.custom = next;
    Ok((transaction, (cursor, run_ids)))
}
