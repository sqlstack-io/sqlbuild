//! Source conditions read the latest warehouse observation of each selected source, a page at a time.

use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroU32;

use chrono::{DateTime, NaiveDateTime, TimeDelta, Utc};
use sqlbuild_recall::records::main::sha256_hex::sha256_hex;

use crate::sensors::constants::{RECENT_EVENT_PAGES, TIMESTAMP_VALUE_KIND};
use crate::sensors::models::{EventScope, NextEvent, SensorCondition, SensorCursor, SensorEvent};
use crate::sensors::types::SourcePage;
use crate::snapshots::main::reads::read_snapshot_nodes::read_snapshot_nodes;
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};
use crate::warehouse_metadata::models::WarehouseFreshnessRecord;

/// The oldest source event on the cursor's page the sensor has not handled, or the next page to read.
pub(crate) async fn next_source_event(
    transaction: ControlTransaction,
    scope: EventScope<'_>,
    (condition, cursor): (&SensorCondition, &SensorCursor),
) -> Result<(ControlTransaction, NextEvent), ControlError> {
    let (mut transaction, (events, next)) =
        page_events(transaction, scope, (condition, cursor.custom.as_deref())).await?;
    for event in events {
        let delivered;
        (transaction, delivered) = was_delivered(transaction, scope, event.id()).await?;
        if !delivered {
            return Ok((
                transaction,
                NextEvent::Found {
                    event,
                    cursor_after: cursor.clone(),
                },
            ));
        }
    }
    let more = next.is_some();
    Ok((
        transaction,
        NextEvent::Nothing {
            cursor: SensorCursor {
                custom: next,
                ..cursor.clone()
            },
            more,
        },
    ))
}

/// Up to `limit` of the selected sources' current events, newest first, whether or not the sensor handled them.
pub(crate) async fn recent_source_events(
    mut transaction: ControlTransaction,
    scope: EventScope<'_>,
    (condition, limit): (&SensorCondition, usize),
) -> Result<(ControlTransaction, Vec<SensorEvent>), ControlError> {
    let mut events: Vec<SensorEvent> = Vec::new();
    let mut start: Option<String> = None;
    for _ in 0..RECENT_EVENT_PAGES {
        let (page, next);
        (transaction, (page, next)) =
            page_events(transaction, scope, (condition, start.as_deref())).await?;
        events.extend(page);
        if next.is_none() {
            break;
        }
        start = next;
    }
    events.sort_by_key(|event| std::cmp::Reverse(event.occurred_at()));
    events.truncate(limit);
    Ok((transaction, events))
}

/// The current event of each source on the page after `start`, and where the next page starts.
async fn page_events(
    transaction: ControlTransaction,
    scope: EventScope<'_>,
    (condition, start): (&SensorCondition, Option<&str>),
) -> Result<(ControlTransaction, (Vec<SensorEvent>, Option<String>)), ControlError> {
    let (selection, stale_after) = match condition {
        SensorCondition::SourceNewData { selection } => (selection, None),
        SensorCondition::SourceStale {
            selection,
            after_seconds,
        } => (selection, Some(*after_seconds)),
        _ => {
            return Err(ControlError::Invalid(
                "a source condition is required".into(),
            ));
        }
    };
    let (mut transaction, (nodes, next)) =
        source_page(transaction, (scope, start), selection).await?;
    let mut events: Vec<SensorEvent> = Vec::new();
    for (key, definition) in nodes {
        let event;
        (transaction, event) = match stale_after {
            None => new_data_event(transaction, scope, (&key, &definition)).await?,
            Some(after_seconds) => {
                stale_event(transaction, scope, (&key, &definition), after_seconds).await?
            }
        };
        events.extend(event);
    }
    Ok((transaction, (events, next)))
}

/// The source's newest version as one event per streak, keyed on the streak's first row so pruning never reuses it.
async fn new_data_event(
    transaction: ControlTransaction,
    scope: EventScope<'_>,
    (key, definition): (&str, &serde_json::Value),
) -> Result<(ControlTransaction, Option<SensorEvent>), ControlError> {
    let (parameters, predicate) = source_scope(scope, key, definition);
    let (transaction, rows) = query(transaction, &format!(
        "SELECT latest.record_json, (SELECT streak.event_id FROM sqb_warehouse_freshness streak \
         WHERE streak.location_id=$1 AND streak.target=$2 AND streak.record_json ->> 'source_name'=$3{streak} \
         AND streak.record_json ->> 'data_version' IS NOT NULL \
         AND streak.record_json ->> 'data_version_hash'=latest.record_json ->> 'data_version_hash' \
         AND NOT EXISTS (SELECT 1 FROM sqb_warehouse_freshness other \
           WHERE other.location_id=$1 AND other.target=$2 AND other.record_json ->> 'source_name'=$3{other} \
           AND other.record_json ->> 'data_version' IS NOT NULL \
           AND other.record_json ->> 'data_version_hash'<>latest.record_json ->> 'data_version_hash' \
           AND (other.received_at,other.observed_at,other.event_id)>(streak.received_at,streak.observed_at,streak.event_id)) \
         ORDER BY streak.received_at,streak.observed_at,streak.event_id LIMIT 1) AS streak_start \
         FROM sqb_warehouse_freshness latest WHERE latest.location_id=$1 AND latest.target=$2 \
         AND latest.record_json ->> 'source_name'=$3{latest} AND latest.record_json ->> 'data_version' IS NOT NULL \
         ORDER BY latest.received_at DESC,latest.observed_at DESC,latest.event_id DESC LIMIT 1",
        streak = predicate.replace("record_json", "streak.record_json"),
        other = predicate.replace("record_json", "other.record_json"),
        latest = predicate.replace("record_json", "latest.record_json"),
    ), parameters).await?;
    let Some(row) = rows.first() else {
        return Ok((transaction, None));
    };
    let record = freshness_record(row)?;
    Ok((
        transaction,
        Some(SensorEvent::SourceNewData {
            id: version_id(("source", key), &record, &row["streak_start"])?,
            occurred_at: record.observed_at,
            source: key.into(),
            data_version: record.data_version,
            target: scope.target.into(),
        }),
    ))
}

/// The source's latest timestamp version, as one event per version, when it is older than `after_seconds` now.
async fn stale_event(
    transaction: ControlTransaction,
    scope: EventScope<'_>,
    (key, definition): (&str, &serde_json::Value),
    after_seconds: NonZeroU32,
) -> Result<(ControlTransaction, Option<SensorEvent>), ControlError> {
    let (parameters, predicate) = source_scope(scope, key, definition);
    let (transaction, rows) = query(transaction, &format!(
        "SELECT record_json FROM sqb_warehouse_freshness WHERE location_id=$1 AND target=$2 \
         AND record_json ->> 'source_name'=$3{predicate} ORDER BY observed_at DESC,event_id DESC LIMIT 1"
    ), parameters).await?;
    let Some(row) = rows.first() else {
        return Ok((transaction, None));
    };
    let record = freshness_record(row)?;
    if !stale_version(&record, transaction.now, after_seconds)? {
        return Ok((transaction, None));
    }
    Ok((
        transaction,
        Some(SensorEvent::SourceStale {
            id: version_id(("stale", key), &record, after_seconds)?,
            occurred_at: record.observed_at,
            source: key.into(),
            data_version: record.data_version,
            after_seconds: after_seconds.get(),
            target: scope.target.into(),
        }),
    ))
}

/// The freshness record a row holds.
fn freshness_record(
    row: &BTreeMap<String, serde_json::Value>,
) -> Result<WarehouseFreshnessRecord, ControlError> {
    serde_json::from_value(row["record_json"].clone())
        .map_err(|error| ControlError::Invalid(error.to_string()))
}

/// A source event's stable ID: `kind`, then a digest of the source, its relation, its version and `also`.
fn version_id(
    (kind, key): (&str, &str),
    record: &WarehouseFreshnessRecord,
    also: impl serde::Serialize,
) -> Result<String, ControlError> {
    let identity = serde_json::to_vec(&(
        key,
        &record.target_database,
        &record.target_schema,
        &record.target_name,
        &record.data_version_hash,
        also,
    ))
    .map_err(|error| ControlError::Invalid(error.to_string()))?;
    Ok(format!("{kind}:{}", sha256_hex(&identity)))
}

/// Whether the sensor already handled the source event `id`; source events have no storage order to pass.
async fn was_delivered(
    transaction: ControlTransaction,
    scope: EventScope<'_>,
    id: &str,
) -> Result<(ControlTransaction, bool), ControlError> {
    let (transaction, rows) = query(
        transaction,
        "SELECT 1 AS delivered FROM sqb_sensor_deliveries WHERE trigger_id=$1 AND event_id=$2",
        vec![Parameter::Id(scope.trigger_id), Parameter::Text(id.into())],
    )
    .await?;
    Ok((transaction, !rows.is_empty()))
}

/// One page of selected source definitions after the cursor's last key, and the next page's start.
async fn source_page(
    transaction: ControlTransaction,
    (scope, start): (EventScope<'_>, Option<&str>),
    selection: &BTreeSet<String>,
) -> Result<SourcePage, ControlError> {
    let mut remaining = selection
        .iter()
        .filter(|key| start.is_none_or(|after| key.as_str() > after));
    let page: BTreeSet<String> = remaining.by_ref().take(100).cloned().collect();
    let next = remaining
        .next()
        .is_some()
        .then(|| page.last().cloned())
        .flatten();
    let (transaction, nodes) =
        read_snapshot_nodes(transaction, scope.snapshot_id, Some(&page)).await?;
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
    scope: EventScope<'_>,
    key: &str,
    definition: &serde_json::Value,
) -> (Vec<Parameter>, String) {
    let mut parameters = vec![
        Parameter::Id(scope.location_id),
        Parameter::Text(scope.target.into()),
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
