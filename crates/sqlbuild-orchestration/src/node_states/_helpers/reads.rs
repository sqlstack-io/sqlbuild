//! One read-only transaction reads the current snapshot's catalog, unless it is already held, and every node's state.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;

use serde_json::{Map, Value};
use uuid::Uuid;

use crate::node_states::_helpers::status::{StatusNode, apply_statuses};
use crate::node_states::models::{ChangedBuild, ChangedBuildNodes, NodeState, SnapshotCatalog};
use crate::snapshots::main::reads::read_current_record::read_current_record;
use crate::snapshots::main::reads::read_snapshot_nodes::read_snapshot_nodes;
use crate::snapshots::models::SnapshotNode;
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{ControlStore, ControlTransaction, Parameter, TransactionMode};
use crate::warehouse_history::main::checks::read_node_check_counts::read_node_check_counts;
use crate::warehouse_metadata::main::node_reads::{
    catalog_node::catalog_node, definition_evidences::definition_evidences,
    destination::destination, last_outcome::last_outcome, latest_attempts::latest_attempts,
    latest_fingerprints::latest_fingerprints,
};
use crate::warehouse_metadata::models::{
    CatalogNodeBuild, CatalogNodeOutcome, DefinitionEvidence, FingerprintQuery,
};

/// The definition fields node states read: build name and destination, planner identity, a source's loader entry.
const STATE_FIELDS: [&str; 4] = ["name", "destination", "planner_identity", "entry"];
/// The snapshot node kinds of a Python loader, which runs under the key of each source it loads, and a source.
const LOADER_KIND: &str = "loader";
const SOURCE_KIND: &str = "source";
const VIEW_MATERIALIZATION: &str = "view";
const FAILED_OUTCOME: &str = "failed";

/// A target's read: its catalog, every node's state, and the changed build planned for the current snapshot.
pub(crate) type TargetRead = (
    Arc<SnapshotCatalog>,
    BTreeMap<String, NodeState>,
    Option<ChangedBuild>,
);

/// The current catalog, reusing `held` while the snapshot is unchanged, its node states and changed build.
pub(crate) async fn read_target(
    store: &ControlStore,
    (location_id, target): (Uuid, &str),
    held: Option<Arc<SnapshotCatalog>>,
) -> Result<TargetRead, ControlError> {
    let transaction = begin(store, TransactionMode::Read).await?;
    let (transaction, snapshot) = read_current_record(transaction, (location_id, target)).await?;
    let snapshot_id = snapshot.map(|snapshot| snapshot.snapshot_id);
    let (transaction, catalog) = match (held, snapshot_id) {
        (Some(held), _) if held.snapshot_id == snapshot_id => (transaction, held),
        (_, None) => (transaction, Arc::new(SnapshotCatalog::default())),
        (_, Some(snapshot_id)) => {
            let (transaction, nodes) = read_snapshot_nodes(transaction, snapshot_id, None).await?;
            (transaction, Arc::new(snapshot_catalog(snapshot_id, nodes)))
        }
    };
    let (transaction, states) = node_states(transaction, (location_id, target), &catalog).await?;
    let (transaction, changed_build) = changed_build(transaction, (location_id, target)).await?;
    commit(transaction).await?;
    Ok((catalog, states, changed_build))
}

/// The location's newest changed build, only when it was planned for the current snapshot's generation.
async fn changed_build(
    transaction: ControlTransaction,
    (location_id, target): (Uuid, &str),
) -> Result<(ControlTransaction, Option<ChangedBuild>), ControlError> {
    let (transaction, rows) = query(
        transaction,
        "SELECT b.generation, b.reported_at, b.report_json FROM sqb_changed_builds b \
         JOIN sqb_current_snapshots c ON c.location_id=b.location_id AND c.target=b.target \
         JOIN sqb_snapshots s ON s.snapshot_id=c.snapshot_id \
         WHERE b.location_id=$1 AND b.target=$2 AND s.generation=b.generation",
        vec![Parameter::Id(location_id), Parameter::Text(target.into())],
    )
    .await?;
    let Some(row) = rows.into_iter().next() else {
        return Ok((transaction, None));
    };
    let invalid =
        |error: serde_json::Error| ControlError::Invalid(format!("stored changed build: {error}"));
    let report: ChangedBuildNodes =
        serde_json::from_value(row["report_json"].clone()).map_err(invalid)?;
    Ok((
        transaction,
        Some(ChangedBuild {
            generation: row["generation"].as_i64().unwrap_or_default(),
            reported_at: serde_json::from_value(row["reported_at"].clone()).map_err(invalid)?,
            changed: report.changed,
            downstream: report.downstream,
            failing: report.failing,
        }),
    ))
}

fn snapshot_catalog(snapshot_id: Uuid, nodes: BTreeMap<String, SnapshotNode>) -> SnapshotCatalog {
    let mut records = Vec::with_capacity(nodes.len());
    let mut inputs = Vec::with_capacity(nodes.len());
    for (key, node) in nodes {
        records.push(catalog_node(
            (&key, &node),
            snapshot_id,
            (None, None, false),
            Default::default(),
        ));
        let definition: Map<String, Value> = STATE_FIELDS
            .iter()
            .filter_map(|field| Some(((*field).to_owned(), node.definition.get(field)?.clone())))
            .collect();
        inputs.push((
            key,
            SnapshotNode {
                kind: node.kind,
                definition: Value::Object(definition),
                raw_sql: None,
                compiled_sql: None,
                python_source: None,
            },
        ));
    }
    SnapshotCatalog {
        snapshot_id: Some(snapshot_id),
        records,
        inputs,
        body: Default::default(),
    }
}

/// Builds are batched indexed seeks; attempts and check results are one read each. Statuses are worked out last, over every node.
async fn node_states(
    transaction: ControlTransaction,
    (location_id, target): (Uuid, &str),
    catalog: &SnapshotCatalog,
) -> Result<(ControlTransaction, BTreeMap<String, NodeState>), ControlError> {
    let nodes: &[(String, SnapshotNode)] = &catalog.inputs;
    let mut positions: Vec<usize> = Vec::new();
    let mut queries: Vec<FingerprintQuery> = Vec::new();
    for (position, (_, node)) in nodes.iter().enumerate() {
        if let Some(query) = destination(node) {
            positions.push(position);
            queries.push(query);
        }
    }
    let (transaction, found) =
        latest_fingerprints(transaction, (location_id, target, &queries)).await?;
    let (transaction, attempts) = latest_attempts(transaction, (location_id, target)).await?;
    let loaded: HashMap<&str, Vec<&str>> = loaded_sources(nodes);
    let (transaction, mut checks) =
        read_node_check_counts(transaction, (location_id, target)).await?;
    let (transaction, evidences) = definition_evidences(
        transaction,
        &positions
            .iter()
            .zip(&found)
            .map(|(position, build)| (&nodes[*position].1, build.as_ref()))
            .collect::<Vec<_>>(),
    )
    .await?;
    let mut builds: Vec<Option<CatalogNodeBuild>> = vec![None; nodes.len()];
    let mut changed: Vec<bool> = vec![false; nodes.len()];
    for ((position, build), evidence) in positions.iter().zip(found).zip(evidences) {
        changed[*position] = matches!(evidence, DefinitionEvidence::Changed { .. });
        builds[*position] = build.map(|(record, _, _)| CatalogNodeBuild {
            observed_at: record.observed_at,
            run_id: record.run_id,
            version_hash: record.version_hash,
        });
    }
    let tables: Vec<bool> = {
        let mut tables = vec![false; nodes.len()];
        positions
            .iter()
            .for_each(|position| tables[*position] = true);
        tables
    };
    let states: BTreeMap<String, NodeState> = nodes
        .iter()
        .zip(builds.into_iter().zip(changed))
        .map(|((key, node), (last_build, definition_changed))| {
            let attempt = newest_attempt(&attempts, key, node, &loaded);
            let last_outcome = last_outcome(attempt, last_build.as_ref());
            let checks = checks
                .remove(key.as_str())
                .or_else(|| checks.remove(&format!("{}:{key}", node.kind)))
                .unwrap_or_default();
            (
                key.clone(),
                NodeState {
                    last_build,
                    last_outcome,
                    definition_changed,
                    checks,
                    ..NodeState::default()
                },
            )
        })
        .collect();
    let listed: HashSet<&str> = nodes.iter().map(|(key, _)| key.as_str()).collect();
    let status_nodes: Vec<StatusNode<'_>> = nodes
        .iter()
        .zip(&catalog.records)
        .zip(tables)
        .map(|(((key, node), record), has_table)| StatusNode {
            key,
            kind: &node.kind,
            has_table,
            view: record.materialization.as_deref() == Some(VIEW_MATERIALIZATION),
            upstream: listed_upstream(&record.dependencies, &listed),
        })
        .collect();
    let states = apply_statuses(&status_nodes, states);
    Ok((transaction, states))
}

/// The dependencies that are themselves listed, as the catalog draws them.
fn listed_upstream<'a>(dependencies: &'a [String], listed: &HashSet<&str>) -> Vec<&'a str> {
    dependencies
        .iter()
        .map(String::as_str)
        .filter(|upstream| listed.contains(upstream))
        .collect()
}

/// Each loader's name and the keys of the sources it loads, whose runs record its work.
pub(crate) fn loaded_sources(nodes: &[(String, SnapshotNode)]) -> HashMap<&str, Vec<&str>> {
    let mut loaded: HashMap<&str, Vec<&str>> = HashMap::new();
    for (key, node) in nodes.iter().filter(|(_, node)| node.kind == SOURCE_KIND) {
        if let Some(loader) = node.definition["entry"]["loader"].as_str() {
            loaded.entry(loader).or_default().push(key);
        }
    }
    loaded
}

/// A node's newest attempt; a loader's is the newest of the sources it loads, a failure first on a tie.
pub(crate) fn newest_attempt(
    attempts: &HashMap<String, CatalogNodeOutcome>,
    key: &str,
    node: &SnapshotNode,
    loaded: &HashMap<&str, Vec<&str>>,
) -> Option<CatalogNodeOutcome> {
    let own = attempts.get(key);
    let sources = (node.kind == LOADER_KIND)
        .then(|| node.definition["name"].as_str())
        .flatten()
        .and_then(|name| loaded.get(name))
        .into_iter()
        .flatten()
        .filter_map(|source| attempts.get(*source));
    own.into_iter()
        .chain(sources)
        .max_by_key(|attempt| (attempt.at, attempt.status == FAILED_OUTCOME))
        .cloned()
}
