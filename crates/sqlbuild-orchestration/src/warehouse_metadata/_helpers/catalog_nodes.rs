//! Catalog listing rows from snapshot node definitions and one latest-build read per target.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

use crate::sensors::constants::SOURCE_NODE_KIND;
use crate::snapshots::models::SnapshotNode;
use crate::store::errors::ControlError;
use crate::store::main::query_records::query_records;
use crate::store::models::{ControlTransaction, Parameter};
use crate::warehouse_metadata::_helpers::node_status::destination;
use crate::warehouse_metadata::models::{
    CatalogNodeBuild, CatalogNodeRecord, CatalogRelation, FingerprintQuery,
};

#[derive(Deserialize)]
struct BuildRow {
    node_type: String,
    node_name: String,
    target_database: Option<String>,
    target_schema: Option<String>,
    target_name: Option<String>,
    observed_at: DateTime<Utc>,
    event_id: Uuid,
    run_id: String,
    version_hash: String,
}

/// The newest fingerprint of every node and relation in the target; ties go to the larger event ID.
pub(crate) async fn latest_builds(
    transaction: ControlTransaction,
    (location_id, target): (Uuid, &str),
) -> Result<
    (
        ControlTransaction,
        BTreeMap<FingerprintQuery, CatalogNodeBuild>,
    ),
    ControlError,
> {
    let (transaction, rows): (_, Vec<BuildRow>) = query_records(transaction,
        "SELECT a.node_type,a.node_name,a.target_database,a.target_schema,a.target_name,a.observed_at,a.event_id, \
           a.record_json ->> 'run_id' AS run_id,a.record_json ->> 'version_hash' AS version_hash \
         FROM sqb_warehouse_fingerprints a JOIN (SELECT node_type,node_name,COALESCE(target_database,'') AS d, \
           COALESCE(target_schema,'') AS s,COALESCE(target_name,'') AS n,MAX(observed_at) AS latest \
           FROM sqb_warehouse_fingerprints WHERE location_id=$1 AND target=$2 AND node_type IS NOT NULL AND node_name IS NOT NULL \
           GROUP BY node_type,node_name,COALESCE(target_database,''),COALESCE(target_schema,''),COALESCE(target_name,'')) l \
         ON a.node_type=l.node_type AND a.node_name=l.node_name AND COALESCE(a.target_database,'')=l.d \
           AND COALESCE(a.target_schema,'')=l.s AND COALESCE(a.target_name,'')=l.n AND a.observed_at=l.latest \
         WHERE a.location_id=$1 AND a.target=$2",
        vec![Parameter::Id(location_id), Parameter::Text(target.into())]).await?;
    let mut builds: BTreeMap<FingerprintQuery, (Uuid, CatalogNodeBuild)> = BTreeMap::new();
    for row in rows {
        let key = FingerprintQuery {
            node_type: row.node_type,
            node_name: row.node_name,
            database: row.target_database,
            schema: row.target_schema,
            name: row.target_name,
        };
        if builds
            .get(&key)
            .is_some_and(|(event_id, _)| *event_id > row.event_id)
        {
            continue;
        }
        let build = CatalogNodeBuild {
            observed_at: row.observed_at,
            run_id: row.run_id,
            version_hash: row.version_hash,
        };
        builds.insert(key, (row.event_id, build));
    }
    let builds: BTreeMap<FingerprintQuery, CatalogNodeBuild> = builds
        .into_iter()
        .map(|(key, (_, build))| (key, build))
        .collect();
    Ok((transaction, builds))
}

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
    builds: &BTreeMap<FingerprintQuery, CatalogNodeBuild>,
) -> CatalogNodeRecord {
    let definition = &node.definition;
    let last_build = destination(node).and_then(|query| builds.get(&query).cloned());
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
