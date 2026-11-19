//! Index each published snapshot once for planning; snapshots never change, so later requests reuse the index.

use std::collections::BTreeMap;
use std::future::Future;
use std::sync::{Arc, Mutex, PoisonError};

use serde_json::Value;
use tokio::sync::OnceCell;
use uuid::Uuid;

use crate::authorization::main::decisions::authorize_person::authorize_person;
use crate::credentials::models::{Action, AuditContext, DefaultRole, PermissionScope};
use crate::declarations::main::frozen_definitions::frozen_definitions;
use crate::runs::constants::PUBLICATIONS_PER_TARGET;
use crate::runs::models::{CachedPublication, PlannerResource, SnapshotSlot, SnapshotSlots};
use crate::runs::types::{CommandPlanner, PublishedJobs};
use crate::snapshots::main::reads::read_current_record::read_current_record;
use crate::snapshots::main::reads::read_kind_nodes::read_kind_nodes;
use crate::snapshots::main::reads::read_snapshot_keys::read_snapshot_keys;
use crate::snapshots::main::write_sets::writer_nodes::writer_nodes;
use crate::snapshots::models::{SnapshotNode, SnapshotRecord};
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit};
use crate::store::models::{ControlStore, ControlTransaction, TransactionMode};

/// Graph kinds in the order the CLI builds its project graph: models, sources, seeds, then functions.
const GRAPH_KINDS: [&str; 5] = ["model", "source", "seed", "udf", "table_fn"];
/// Python node kinds, in the order the CLI discovers them.
const PYTHON_NODE_KINDS: [&str; 4] = ["task", "asset", "loader", "check"];
/// SQL unit tests, which `test:` selects; each lists the models it tests.
const SQL_TEST_KIND: &str = "sql_test";
/// Every kind planning reads a definition of; audits, macros and scenarios are neither selected nor written.
const PLANNED_KINDS: [&str; 10] = [
    "model", "source", "seed", "udf", "table_fn", "task", "asset", "loader", "check", "sql_test",
];
/// Where a SQL unit test's definition lists the models it tests.
const SQL_TEST_MODELS_POINTER: &str = "/test/payload/expected_model_names";
/// Marks the source node an intermediate loader writes, which the CLI's SQL graph does not list.
const INTERMEDIATE_LOADER_FIELD: &str = "intermediate_loader";

/// The target's current snapshot and its prepared publication, or `None` before its first publication.
pub(crate) async fn current_publication(
    transaction: ControlTransaction,
    planner: &dyn CommandPlanner,
    (location_id, target): (Uuid, &str),
) -> Result<
    (
        ControlTransaction,
        Option<(SnapshotRecord, Arc<CachedPublication>)>,
    ),
    ControlError,
> {
    let (transaction, snapshot) = read_current_record(transaction, (location_id, target)).await?;
    let Some(snapshot) = snapshot else {
        return Ok((transaction, None));
    };
    let (transaction, publication) = cached_publication(
        transaction,
        planner,
        ((location_id, target), snapshot.snapshot_id),
    )
    .await?;
    Ok((transaction, Some((snapshot, publication))))
}

/// The current publication for a person with view on every one of its nodes, as the node catalog requires; a denial is audited.
pub(crate) async fn viewed_publication(
    store: &ControlStore,
    (planner, credentials): (&dyn CommandPlanner, (&AuditContext, Uuid)),
    (location_id, target): (Uuid, &str),
    default_role: DefaultRole,
) -> Result<Option<(SnapshotRecord, Arc<CachedPublication>)>, ControlError> {
    let transaction = begin(store, TransactionMode::Write).await?;
    let (transaction, current) =
        current_publication(transaction, planner, (location_id, target)).await?;
    let scope = PermissionScope {
        action: Action::View,
        location_id: Some(location_id),
        target: Some(target.into()),
        nodes: current
            .as_ref()
            .map(|(_, publication)| publication.node_keys.clone())
            .unwrap_or_default(),
    };
    let (transaction, _) = authorize_person(transaction, credentials, &scope, default_role).await?;
    commit(transaction).await?;
    Ok(current)
}

/// One snapshot's prepared publication, read from the store only by the first request that needs it.
pub(crate) async fn cached_publication(
    transaction: ControlTransaction,
    planner: &dyn CommandPlanner,
    ((location_id, target), snapshot_id): ((Uuid, &str), Uuid),
) -> Result<(ControlTransaction, Arc<CachedPublication>), ControlError> {
    cached(
        &planner.publications().slots,
        ((location_id, target.to_owned()), snapshot_id),
        transaction,
        |transaction| read_publication(transaction, planner, snapshot_id),
    )
    .await
}

/// The publication cached for `snapshot_id`, built once with `build` while concurrent callers wait; a failed build caches nothing.
pub(crate) async fn cached<S, F, Fut>(
    slots: &Mutex<SnapshotSlots>,
    key: ((Uuid, String), Uuid),
    state: S,
    build: F,
) -> Result<(S, Arc<CachedPublication>), ControlError>
where
    F: FnOnce(S) -> Fut,
    Fut: Future<Output = Result<(S, CachedPublication), ControlError>>,
{
    let cell = slot(slots, key);
    let mut state = Some(state);
    let value = cell
        .get_or_try_init(|| async {
            let given = state
                .take()
                .ok_or_else(|| ControlError::Invalid("publication state already taken".into()))?;
            let (returned, value) = build(given).await?;
            state = Some(returned);
            Ok::<_, ControlError>(Arc::new(value))
        })
        .await?
        .clone();
    let state =
        state.ok_or_else(|| ControlError::Invalid("publication state already taken".into()))?;
    Ok((state, value))
}

/// The snapshot's cell, marked most recently used; adding one evicts its scope's least recently used beyond the limit.
fn slot(
    slots: &Mutex<SnapshotSlots>,
    (scope, snapshot_id): ((Uuid, String), Uuid),
) -> Arc<OnceCell<Arc<CachedPublication>>> {
    let mut state = slots.lock().unwrap_or_else(PoisonError::into_inner);
    state.clock += 1;
    let now = state.clock;
    if let Some(slot) = state
        .slots
        .iter_mut()
        .find(|slot| slot.snapshot_id == snapshot_id)
    {
        slot.used = now;
        return Arc::clone(&slot.cell);
    }
    let cell = Arc::new(OnceCell::new());
    state.slots.push(SnapshotSlot {
        scope: scope.clone(),
        snapshot_id,
        used: now,
        cell: Arc::clone(&cell),
    });
    loop {
        let in_scope = state
            .slots
            .iter()
            .enumerate()
            .filter(|(_, slot)| slot.scope == scope);
        if in_scope.clone().count() <= PUBLICATIONS_PER_TARGET {
            break;
        }
        let Some(oldest) = in_scope
            .min_by_key(|(_, slot)| slot.used)
            .map(|(index, _)| index)
        else {
            break;
        };
        state.slots.remove(oldest);
    }
    cell
}

/// Read what planning needs from one snapshot and index it with the planner.
async fn read_publication(
    transaction: ControlTransaction,
    planner: &dyn CommandPlanner,
    snapshot_id: Uuid,
) -> Result<(ControlTransaction, CachedPublication), ControlError> {
    let (transaction, node_keys) = read_snapshot_keys(transaction, snapshot_id).await?;
    let (transaction, nodes) = read_kind_nodes(transaction, snapshot_id, &PLANNED_KINDS).await?;
    let (transaction, definitions) = frozen_definitions(transaction, snapshot_id).await?;
    let jobs: PublishedJobs = definitions
        .jobs
        .into_iter()
        .map(|(name, job)| (name, job.selection))
        .chain(
            definitions
                .groups
                .into_iter()
                .map(|(name, group)| (name, group.job.selection)),
        )
        .collect();
    Ok((
        transaction,
        CachedPublication {
            prepared: planner.prepare(planner_resources(&nodes), jobs),
            writers: writer_nodes(&nodes),
            node_keys,
        },
    ))
}

/// The selector graph rebuilt from published definitions, in the CLI's kind order; sources intermediate loaders write are selected through their loader.
fn planner_resources(nodes: &BTreeMap<String, SnapshotNode>) -> Vec<PlannerResource> {
    let mut resources: Vec<PlannerResource> = Vec::new();
    for kind in GRAPH_KINDS.iter().chain(&PYTHON_NODE_KINDS) {
        for (key, node) in nodes {
            let definition = &node.definition;
            if node.kind != *kind || definition.get(INTERMEDIATE_LOADER_FIELD).is_some() {
                continue;
            }
            let (tags, folder) = match *kind {
                "model" => (
                    definition.pointer("/config/tags"),
                    Some(model_folder(definition)),
                ),
                "seed" => (definition.pointer("/schema/tags"), None),
                _ if PYTHON_NODE_KINDS.contains(kind) => {
                    (definition.get("tags"), Some(model_folder(definition)))
                }
                _ => (definition.get("tags"), None),
            };
            resources.push(PlannerResource {
                key: key.clone(),
                deps: texts(definition.get("deps")),
                tags: texts(tags),
                folder,
                loader: definition
                    .pointer("/entry/loader")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
            });
        }
    }
    resources.extend(
        nodes
            .iter()
            .filter(|(_, node)| node.kind == SQL_TEST_KIND)
            .map(|(key, node)| PlannerResource {
                key: key.clone(),
                deps: tested_models(&node.definition),
                tags: Vec::new(),
                folder: None,
                loader: None,
            }),
    );
    resources
}

/// The model file's folder relative to the project, with `/` separators whichever platform published it.
fn model_folder(definition: &Value) -> String {
    let path = definition
        .get("path")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .replace('\\', "/");
    path.rsplit_once('/')
        .map_or_else(|| ".".to_owned(), |(folder, _)| folder.to_owned())
}

/// The model keys a SQL unit test tests.
fn tested_models(definition: &Value) -> Vec<String> {
    texts(definition.pointer(SQL_TEST_MODELS_POINTER))
        .into_iter()
        .map(|model| format!("model:{model}"))
        .collect()
}

fn texts(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| item.as_str().map(str::to_owned))
        .collect()
}
