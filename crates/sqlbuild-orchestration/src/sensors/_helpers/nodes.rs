//! Node conditions consume immutable execution evidence or warehouse source observations.

use std::collections::BTreeSet;
use std::num::NonZeroU32;

use chrono::{DateTime, NaiveDateTime, TimeDelta, Utc};
use sqlbuild_recall::records::main::sha256_hex::sha256_hex;
use uuid::Uuid;

use crate::runs::models::{NodeOutcome, RunChange, RunChangeRequest};
use crate::sensors::_helpers::requests::queue_sensor_run;
use crate::sensors::_helpers::transitions::matches_transition;
use crate::sensors::constants::TIMESTAMP_VALUE_KIND;
use crate::sensors::models::{
    NodeEventKind, NodeSensorEvent, SensorCursor, SensorEvaluationRequest, SourceSensorEvent,
};
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
        "SELECT storage_id,event_id,run_id,payload FROM recall_events \
         WHERE producer='scheduler' AND event_type='run_node_finished' \
         AND invocation_id='scheduler/' || run_id AND storage_id>$1 \
         ORDER BY storage_id LIMIT 100",
        vec![Parameter::Integer(evaluation.cursor.event_storage_id)],
    )
    .await?;
    let mut cursor = evaluation.cursor.clone();
    let mut run_ids: Vec<Uuid> = Vec::new();
    for row in rows {
        let event: NodeSensorEvent = decode_row(row)?;
        cursor.event_storage_id = event.storage_id;
        let request: RunChangeRequest = serde_json::from_value(event.payload["change"].clone())
            .map_err(|error| ControlError::Invalid(error.to_string()))?;
        if request.run_id != event.run_id || request.event_id != event.event_id {
            return Err(ControlError::Invalid("node event identity mismatch".into()));
        }
        let RunChange::NodeFinished {
            node_key, outcome, ..
        } = &request.change
        else {
            continue;
        };
        if !selection.contains(node_key)
            || !matches!(outcome, NodeOutcome::Success | NodeOutcome::Failed)
        {
            continue;
        }
        let started;
        (transaction, started) =
            prior_start(transaction, evaluation, &request, event.storage_id).await?;
        if !started {
            continue;
        }
        let matches;
        (transaction, matches) = matches_transition(
            transaction,
            evaluation,
            (node_key, *outcome, event.storage_id),
            kinds,
        )
        .await?;
        if matches {
            let run_id;
            (transaction, run_id) =
                queue_sensor_run(transaction, evaluation, &event.event_id.to_string()).await?;
            run_ids.push(run_id);
        }
    }
    Ok((transaction, (cursor, run_ids)))
}

pub(crate) async fn consume_source_events(
    transaction: ControlTransaction,
    evaluation: &SensorEvaluationRequest,
    selection: &BTreeSet<String>,
) -> Result<(ControlTransaction, (SensorCursor, Vec<Uuid>)), ControlError> {
    let (mut transaction, rows) = query(
        transaction,
        "SELECT storage_id,event_id,invocation_id,invocation_sequence,payload FROM recall_events \
         WHERE producer='warehouse' AND event_type='warehouse_source_freshness' \
         AND invocation_id LIKE 'warehouse/source-freshness/%' AND storage_id>$1 \
         ORDER BY storage_id LIMIT 100",
        vec![Parameter::Integer(evaluation.cursor.event_storage_id)],
    )
    .await?;
    let mut cursor = evaluation.cursor.clone();
    let mut run_ids: Vec<Uuid> = Vec::new();
    for row in rows {
        let event: SourceSensorEvent = decode_row(row)?;
        cursor.event_storage_id = event.storage_id;
        let observation = &event.payload;
        let record = &observation.record;
        if observation.location_id != evaluation.location_id
            || observation.target != evaluation.target
            || !selection.contains(&format!("source:{}", record.source_name))
            || record.data_version.is_none()
        {
            continue;
        }
        let previous;
        (transaction, previous) = query(
            transaction,
            "SELECT payload -> 'record' ->> 'data_version_hash' AS hash FROM recall_events \
             WHERE producer='warehouse' AND event_type='warehouse_source_freshness' \
             AND invocation_id=$1 AND invocation_sequence=$2 LIMIT 1",
            vec![
                Parameter::Text(event.invocation_id),
                Parameter::Integer(event.invocation_sequence - 1),
            ],
        )
        .await?;
        if previous
            .first()
            .is_some_and(|row| row["hash"] == record.data_version_hash)
        {
            continue;
        }
        let run_id;
        (transaction, run_id) =
            queue_sensor_run(transaction, evaluation, &event.event_id.to_string()).await?;
        run_ids.push(run_id);
    }
    Ok((transaction, (cursor, run_ids)))
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
    let mut cursor = evaluation.cursor.clone();
    let mut remaining = selection
        .iter()
        .filter(|key| cursor.custom.as_ref().is_none_or(|after| *key > after));
    let page: BTreeSet<String> = remaining.by_ref().take(100).cloned().collect();
    let next = remaining
        .next()
        .is_some()
        .then(|| page.last().cloned())
        .flatten();
    let (mut transaction, nodes) =
        read_snapshot_nodes(transaction, evaluation.snapshot_id, Some(&page)).await?;
    let mut run_ids: Vec<Uuid> = Vec::new();
    for (key, node) in nodes {
        let mut parameters = vec![
            Parameter::Id(evaluation.location_id),
            Parameter::Text(evaluation.target.clone()),
            Parameter::Text(key.replacen("source:", "", 1)),
        ];
        let mut predicate = String::new();
        for (column, value) in [
            (
                "target_database",
                node.definition["namespace"]
                    .get(0)
                    .unwrap_or(&node.definition["database"]),
            ),
            (
                "target_schema",
                node.definition["namespace"]
                    .get(1)
                    .unwrap_or(&node.definition["schema"]),
            ),
            ("target_name", &node.definition["table"]),
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
    cursor.custom = next;
    Ok((transaction, (cursor, run_ids)))
}

async fn prior_start(
    transaction: ControlTransaction,
    evaluation: &SensorEvaluationRequest,
    request: &RunChangeRequest,
    storage_id: i64,
) -> Result<(ControlTransaction, bool), ControlError> {
    let (RunChange::NodeFinished { node_key, .. }, Some(claim_id)) =
        (&request.change, request.claim_id)
    else {
        return Ok((transaction, false));
    };
    let (transaction, rows) = query(
        transaction,
        "SELECT s.event_id FROM sqb_runs r JOIN recall_events s ON s.run_id=CAST(r.run_id AS TEXT) \
         WHERE s.invocation_id=$1 AND s.run_id=$2 AND s.producer='scheduler' \
         AND s.event_type='run_node_started' AND s.storage_id<$3 \
         AND (s.payload -> 'change' ->> 'claim_id')=$4 \
         AND (s.payload -> 'change' -> 'change' ->> 'node_key')=$5 \
         AND r.run_id=$6 AND r.location_id=$7 AND r.target=$8 AND r.kind<>'group' LIMIT 1",
        vec![
            Parameter::Text(format!("scheduler/{}", request.run_id)),
            Parameter::Text(request.run_id.to_string()),
            Parameter::Integer(storage_id),
            Parameter::Text(claim_id.to_string()),
            Parameter::Text(node_key.clone()),
            Parameter::Id(request.run_id),
            Parameter::Id(evaluation.location_id),
            Parameter::Text(evaluation.target.clone()),
        ],
    )
    .await?;
    Ok((transaction, !rows.is_empty()))
}
