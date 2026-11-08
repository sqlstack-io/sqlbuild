//! Catalog listing rows and column lineage from snapshot node definitions and published edges.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use serde_json::Value;
use uuid::Uuid;

use crate::sensors::constants::SOURCE_NODE_KIND;
use crate::snapshots::models::SnapshotNode;
use crate::warehouse_metadata::models::{
    CatalogNodeBuild, CatalogNodeRecord, CatalogRelation, ColumnLineageEdge, LineageColumn,
    LineageNodeColumns,
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
    last_build: Option<CatalogNodeBuild>,
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
        snapshot_id,
    }
}

/// Published edge rows whose both ends are current nodes, so the graph never points at a hidden node.
pub(crate) fn published_edges(
    document: Option<&Value>,
    nodes: &BTreeMap<String, SnapshotNode>,
) -> Vec<ColumnLineageEdge> {
    let rows = document
        .and_then(|document| document.get("edges"))
        .and_then(Value::as_array);
    rows.into_iter()
        .flatten()
        .filter_map(edge_row)
        .filter(|edge| nodes.contains_key(&edge.source_key) && nodes.contains_key(&edge.target_key))
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

/// The inferred columns of the focal node and of every node an edge touches, in node key order.
pub(crate) fn node_columns(
    nodes: &BTreeMap<String, SnapshotNode>,
    node_key: &str,
    edges: &[ColumnLineageEdge],
) -> Vec<LineageNodeColumns> {
    let mut keys: BTreeSet<&str> = BTreeSet::from([node_key]);
    for edge in edges {
        keys.extend([edge.source_key.as_str(), edge.target_key.as_str()]);
    }
    keys.into_iter()
        .filter_map(|key| nodes.get_key_value(key))
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
