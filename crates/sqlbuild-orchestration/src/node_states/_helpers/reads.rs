//! One read-only transaction reads the current snapshot's catalog, unless it is already held, and every node's state.

use std::collections::BTreeMap;
use std::sync::Arc;

use serde_json::{Map, Value};
use uuid::Uuid;

use crate::node_states::models::{ChangedBuild, ChangedBuildNodes, NodeState, SnapshotCatalog};
use crate::snapshots::main::reads::read_current_record::read_current_record;
use crate::snapshots::main::reads::read_snapshot_nodes::read_snapshot_nodes;
use crate::snapshots::models::SnapshotNode;
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{ControlStore, ControlTransaction, Parameter, TransactionMode};
use crate::warehouse_history::main::read_node_check_counts::read_node_check_counts;
use crate::warehouse_metadata::main::node_reads::{
    catalog_node::catalog_node, definition_evidences::definition_evidences,
    destination::destination, last_outcome::last_outcome, latest_attempts::latest_attempts,
    latest_fingerprints::latest_fingerprints,
};
use crate::warehouse_metadata::models::{CatalogNodeBuild, DefinitionEvidence, FingerprintQuery};

/// The definition fields node states read: the name and destination of builds, and the planner identity.
const STATE_FIELDS: [&str; 3] = ["name", "destination", "planner_identity"];

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
    let (transaction, states) =
        node_states(transaction, (location_id, target), &catalog.inputs).await?;
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

/// Builds are batched indexed seeks; attempts and check results are one read each.
async fn node_states(
    transaction: ControlTransaction,
    (location_id, target): (Uuid, &str),
    nodes: &[(String, SnapshotNode)],
) -> Result<(ControlTransaction, BTreeMap<String, NodeState>), ControlError> {
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
    let (transaction, mut attempts) = latest_attempts(transaction, (location_id, target)).await?;
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
    let states: BTreeMap<String, NodeState> = nodes
        .iter()
        .zip(builds.into_iter().zip(changed))
        .map(|((key, node), (last_build, definition_changed))| {
            let last_outcome = last_outcome(attempts.remove(key.as_str()), last_build.as_ref());
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
                },
            )
        })
        .collect();
    Ok((transaction, states))
}
