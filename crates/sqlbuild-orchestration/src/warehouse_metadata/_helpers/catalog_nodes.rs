//! Catalog listing rows from snapshot node definitions and each node's latest build.

use serde_json::Value;
use uuid::Uuid;

use crate::sensors::constants::SOURCE_NODE_KIND;
use crate::snapshots::models::SnapshotNode;
use crate::warehouse_metadata::models::{CatalogNodeBuild, CatalogNodeRecord, CatalogRelation};

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
