//! Catalog listing rows with each node's newest attempt, and column lineage from published edges.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

use crate::sensors::constants::SOURCE_NODE_KIND;
use crate::snapshots::main::read_snapshot_nodes::read_snapshot_nodes;
use crate::snapshots::models::SnapshotNode;
use crate::store::errors::ControlError;
use crate::store::main::query_records::query_records;
use crate::store::models::{ControlTransaction, Parameter};
use crate::warehouse_history::_helpers::checks::AuditRow;
use crate::warehouse_metadata::constants::{
    LINEAGE_DEFINITION_BATCH, OUTCOME_FAILURE_WINDOW, OUTCOME_RECENT_RUNS,
};
use crate::warehouse_metadata::models::{
    CatalogNodeBuild, CatalogNodeChecks, CatalogNodeOutcome, CatalogNodeRecord, CatalogRelation,
    ColumnLineageEdge, LineageColumn, LineageNodeColumns,
};

/// A declared value from the definition itself, its schema entry or its config, in that order.
fn declared<'a>(definition: &'a Value, key: &str) -> Option<&'a Value> {
    [
        definition.get(key),
        definition.get("schema").and_then(|entry| entry.get(key)),
        definition.get("config").and_then(|config| config.get(key)),
    ]
    .into_iter()
    .flatten()
    .find(|value| !value.is_null())
}

fn texts(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::String(text)) => vec![text.clone()],
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|item| item.as_str().map(str::to_owned))
            .collect(),
        _ => Vec::new(),
    }
}

fn text(value: Option<&Value>) -> Option<String> {
    value.and_then(Value::as_str).map(str::to_owned)
}

fn relation(node: &SnapshotNode) -> Option<CatalogRelation> {
    let definition = &node.definition;
    let (relation, name_key) = match definition.get("destination") {
        Some(destination) if !destination.is_null() => (destination, "name"),
        _ if node.kind == SOURCE_NODE_KIND => (definition, "table"),
        _ => return None,
    };
    Some(CatalogRelation {
        database: text(relation.get("database")),
        schema: text(relation.get("schema"))?,
        name: text(relation.get(name_key))?,
    })
}

pub(crate) fn catalog_node(
    (node_key, node): (&str, &SnapshotNode),
    snapshot_id: Uuid,
    (last_build, last_outcome, definition_changed, checks): (
        Option<CatalogNodeBuild>,
        Option<CatalogNodeOutcome>,
        bool,
        CatalogNodeChecks,
    ),
) -> CatalogNodeRecord {
    let definition = &node.definition;
    CatalogNodeRecord {
        node_key: node_key.into(),
        kind: node.kind.clone(),
        name: text(definition.get("name")).unwrap_or_else(|| {
            node_key
                .split_once(':')
                .map_or(node_key, |(_, name)| name)
                .into()
        }),
        path: text(definition.get("path")),
        materialization: text(definition.get("materialization")),
        description: text(declared(definition, "description")),
        tags: texts(declared(definition, "tags")),
        dependencies: texts(definition.get("deps")),
        relation: relation(node),
        column_count: match definition.get("columns") {
            Some(Value::Array(columns)) => Some(columns.len()),
            Some(Value::Object(columns)) => Some(columns.len()),
            _ => None,
        },
        last_build,
        last_outcome,
        definition_changed,
        checks,
        snapshot_id,
    }
}

/// Each node's checks, keyed `kind:name` like the node keys, from every check's newest result.
pub(crate) fn node_checks(latest: Vec<AuditRow>) -> HashMap<String, CatalogNodeChecks> {
    let mut counts: HashMap<String, CatalogNodeChecks> = HashMap::new();
    for row in latest {
        let record = row.record;
        let (Some(kind), Some(name)) = (record.attached_target_kind, record.attached_target_name)
        else {
            continue;
        };
        let checks = counts.entry(format!("{kind}:{name}")).or_default();
        checks.total += 1;
        match record.outcome.as_str() {
            "pass" => checks.passing += 1,
            "warn" => checks.warning += 1,
            "fail" | "error" => checks.failing += 1,
            _ => {}
        }
    }
    counts
}

#[derive(Deserialize)]
struct Attempt {
    node_key: String,
    run_id: Uuid,
    status: String,
    ended_at: Option<DateTime<Utc>>,
    finished_storage_id: i64,
}

/// Each node's newest started `success`, `warn` or `failed` attempt in those runs, excluding group runs.
pub(crate) async fn latest_attempts(
    transaction: ControlTransaction,
    (location_id, target): (Uuid, &str),
) -> Result<(ControlTransaction, HashMap<String, CatalogNodeOutcome>), ControlError> {
    let newest = "SELECT run_id, status, queued_at FROM sqb_runs WHERE location_id=$1 AND target=$2 AND kind<>'group' \
        ORDER BY queued_at DESC, run_id DESC";
    let (transaction, attempts): (_, Vec<Attempt>) = query_records(transaction,
        &format!("SELECT n.node_key,n.run_id,n.status,n.ended_at,n.finished_storage_id FROM (\
            SELECT run_id FROM ({newest} LIMIT $3) AS latest UNION \
            SELECT run_id FROM (SELECT run_id FROM ({newest} LIMIT $4) AS recent WHERE status IN ('failed','partial','warn') \
            ORDER BY queued_at DESC, run_id DESC LIMIT $3) AS failures) AS r \
         JOIN sqb_run_nodes n ON n.run_id=r.run_id WHERE n.kind='node' AND n.finished_storage_id IS NOT NULL \
         AND n.started_at IS NOT NULL AND n.status IN ('success','warn','failed')"),
        vec![Parameter::Id(location_id), Parameter::Text(target.into()), Parameter::Integer(OUTCOME_RECENT_RUNS), Parameter::Integer(OUTCOME_FAILURE_WINDOW)]).await?;
    let mut newest: HashMap<String, Attempt> = HashMap::new();
    for attempt in attempts {
        if newest
            .get(&attempt.node_key)
            .is_none_or(|seen| attempt.finished_storage_id > seen.finished_storage_id)
        {
            newest.insert(attempt.node_key.clone(), attempt);
        }
    }
    let outcomes: HashMap<String, CatalogNodeOutcome> = newest
        .into_iter()
        .filter_map(|(key, attempt)| {
            Some((
                key,
                CatalogNodeOutcome {
                    status: attempt.status,
                    at: attempt.ended_at?,
                    run_id: Some(attempt.run_id),
                },
            ))
        })
        .collect();
    Ok((transaction, outcomes))
}

/// The scheduler attempt, unless a fingerprint from another clock was recorded after it ended.
pub(crate) fn last_outcome(
    attempt: Option<CatalogNodeOutcome>,
    build: Option<&CatalogNodeBuild>,
) -> Option<CatalogNodeOutcome> {
    let newer_build =
        build.filter(|build| attempt.as_ref().is_none_or(|a| build.observed_at > a.at));
    match newer_build {
        Some(build) => Some(CatalogNodeOutcome {
            status: "success".into(),
            at: build.observed_at,
            run_id: None,
        }),
        None => attempt,
    }
}

/// Published edge rows whose both ends are current nodes, so the graph never points at a hidden node.
pub(crate) fn published_edges(
    document: Option<&Value>,
    nodes: &BTreeSet<String>,
) -> Vec<ColumnLineageEdge> {
    let rows = document
        .and_then(|document| document.get("edges"))
        .and_then(Value::as_array);
    rows.into_iter()
        .flatten()
        .filter_map(edge_row)
        .filter(|edge| nodes.contains(&edge.source_key) && nodes.contains(&edge.target_key))
        .collect()
}

fn edge_row(row: &Value) -> Option<ColumnLineageEdge> {
    let [
        source_key,
        source_column,
        target_key,
        target_column,
        transform,
    ] = row.as_array()?.as_slice()
    else {
        return None;
    };
    let text = |value: &Value| value.as_str().map(str::to_owned);
    Some(ColumnLineageEdge {
        source_key: text(source_key)?,
        source_column: text(source_column)?,
        target_key: text(target_key)?,
        target_column: text(target_column)?,
        transform: text(transform)?,
    })
}

/// Edges reached upstream and downstream from any column of `node_key`, each once, in published order.
pub(crate) fn neighbourhood(edges: &[ColumnLineageEdge], node_key: &str) -> Vec<ColumnLineageEdge> {
    let mut reached: BTreeSet<usize> = walk(edges, node_key, true);
    reached.extend(walk(edges, node_key, false));
    reached
        .into_iter()
        .map(|index| edges[index].clone())
        .collect()
}

/// The edges followed from the node's columns towards sources (`upstream`) or consumers.
fn walk(edges: &[ColumnLineageEdge], node_key: &str, upstream: bool) -> BTreeSet<usize> {
    let mut reached: BTreeSet<usize> = BTreeSet::new();
    let mut by_near: HashMap<(&str, &str), Vec<usize>> = HashMap::new();
    for (index, edge) in edges.iter().enumerate() {
        by_near.entry(end(edge, !upstream)).or_default().push(index);
    }
    let mut pending: Vec<(&str, &str)> = by_near
        .keys()
        .filter(|(key, _)| *key == node_key)
        .copied()
        .collect();
    let mut seen: BTreeSet<(&str, &str)> = pending.iter().copied().collect();
    while let Some(column) = pending.pop() {
        for &index in by_near.get(&column).into_iter().flatten() {
            reached.insert(index);
            let far = end(&edges[index], upstream);
            if seen.insert(far) {
                pending.push(far);
            }
        }
    }
    reached
}

/// An edge's source column, or its target column when `source` is false.
fn end(edge: &ColumnLineageEdge, source: bool) -> (&str, &str) {
    if source {
        (&edge.source_key, &edge.source_column)
    } else {
        (&edge.target_key, &edge.target_column)
    }
}

/// The focal node and every node an edge touches, whose definitions the read loads.
pub(crate) fn touched_nodes(node_key: &str, edges: &[ColumnLineageEdge]) -> BTreeSet<String> {
    let mut keys: BTreeSet<String> = BTreeSet::from([node_key.to_owned()]);
    for edge in edges {
        keys.extend([edge.source_key.clone(), edge.target_key.clone()]);
    }
    keys
}

/// The inferred columns of each loaded node, in node key order.
pub(crate) fn node_columns(nodes: &BTreeMap<String, SnapshotNode>) -> Vec<LineageNodeColumns> {
    nodes
        .iter()
        .map(|(key, node)| LineageNodeColumns {
            node_key: key.clone(),
            kind: node.kind.clone(),
            columns: definition_columns(&node.definition),
        })
        .collect()
}

fn definition_columns(definition: &Value) -> Vec<LineageColumn> {
    let columns = definition.get("columns").and_then(Value::as_array);
    columns
        .into_iter()
        .flatten()
        .filter_map(|column| {
            Some(LineageColumn {
                name: column.get("name")?.as_str()?.to_owned(),
                data_type: column
                    .get("type")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
            })
        })
        .collect()
}

/// The definitions of `keys`, read in bounded batches because a large snapshot's definitions dominate the read.
pub(crate) async fn touched_definitions(
    mut transaction: ControlTransaction,
    snapshot_id: Uuid,
    keys: &BTreeSet<String>,
) -> Result<(ControlTransaction, BTreeMap<String, SnapshotNode>), ControlError> {
    let keys: Vec<&String> = keys.iter().collect();
    let mut nodes: BTreeMap<String, SnapshotNode> = BTreeMap::new();
    for batch in keys.chunks(LINEAGE_DEFINITION_BATCH) {
        let batch: BTreeSet<String> = batch.iter().map(|key| (*key).clone()).collect();
        let (next, loaded) = read_snapshot_nodes(transaction, snapshot_id, Some(&batch)).await?;
        transaction = next;
        nodes.extend(loaded);
    }
    Ok((transaction, nodes))
}
