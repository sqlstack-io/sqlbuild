//! Record what each model and seed execution consumed, and decide `--changed` skips, around one build.

use crate::consumption::models::ChangedDecision;
use crate::consumption::models::ConsumptionRecorder;
use crate::consumption::models::Prepared;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use sqlbuild_state::consumption::constants::RECENT_EXECUTIONS;
use sqlbuild_state::consumption::main::changed_run_set::changed_run_set;
use sqlbuild_state::consumption::main::consumption_history::consumption_history;
use sqlbuild_state::consumption::main::read_back_history::read_back_history;
use sqlbuild_state::consumption::main::started_append::started_append;
use sqlbuild_state::consumption::models::{
    ChangedOutcome, ChangedPolicy, ConsumptionAppend, ConsumptionHistory, ConsumptionRow, HeldBack,
    InputFact, NodeFacts, NodeKey, Reach, RelationKey, RunReason,
};
use uuid::Uuid;

use crate::consumption::_helpers::facts::{BuildFacts, MODEL, SEED, build_facts};
use sqlbuild_python_front::native_host::models::{
    ConsumptionConnection, Location, NodeCompleteHook,
};

/// Succeeded rows are flushed once this many executions are pending.
pub(crate) const FLUSH_EXECUTIONS: usize = 100;
/// Or once this long has passed since the last flush.
pub(crate) const FLUSH_INTERVAL: Duration = Duration::from_secs(30);
const MODEL_RESULT: &str = "model_name";
const SEED_RESULT: &str = "seed_name";
pub(crate) const SUCCESS: &str = "success";
pub(crate) const ERROR_OUTCOME: &str = "error";

/// The mutable side of one build's recording.
#[derive(Debug)]
pub(crate) struct Recording {
    pub(crate) pending: BTreeMap<Location, Vec<ConsumptionAppend>>,
    pub(crate) pending_executions: usize,
    pub(crate) pending_seeds: Vec<NodeKey>,
    pub(crate) tables: BTreeSet<Location>,
    pub(crate) last_flush: Instant,
    pub(crate) warnings: Vec<String>,
}

impl Default for Recording {
    fn default() -> Self {
        Self {
            pending: BTreeMap::new(),
            pending_executions: 0,
            pending_seeds: Vec::new(),
            tables: BTreeSet::new(),
            last_flush: Instant::now(),
            warnings: Vec::new(),
        }
    }
}

pub(crate) fn kind_of(result: &Bound<'_, PyAny>) -> PyResult<Option<(&'static str, String)>> {
    for (field, kind) in [(MODEL_RESULT, MODEL), (SEED_RESULT, SEED)] {
        if let Some(name) = result.getattr_opt(field)? {
            return Ok(Some((kind, name.extract()?)));
        }
    }
    Ok(None)
}

/// Schemas to read records from: all under `--changed`, else those of unselected upstreams.
fn read_locations(facts: &BuildFacts, changed: bool) -> BTreeSet<Location> {
    let by_key: HashMap<&NodeKey, &NodeFacts> =
        facts.nodes.iter().map(|node| (&node.key, node)).collect();
    if changed {
        return facts
            .nodes
            .iter()
            .map(|node| Location::of(&node.relation))
            .collect();
    }
    facts
        .nodes
        .iter()
        .filter(|node| facts.runnable.contains(&node.key))
        .flat_map(|node| node.inputs.iter())
        .filter_map(|input| match input {
            InputFact::Node(upstream) if !facts.runnable.contains(upstream) => by_key.get(upstream),
            _ => None,
        })
        .map(|node| Location::of(&node.relation))
        .collect()
}

/// Read records, decide `--changed` skips, mint execution IDs and append `started` rows.
pub(crate) fn prepare<'py>(
    (plan, project): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    (adapter, connection_config): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    (policy, full_refresh): (Option<&ChangedPolicy>, bool),
    (run_id, base_on_node_complete): (String, Bound<'py, PyAny>),
) -> PyResult<Prepared<'py>> {
    let py: Python<'py> = plan.py();
    let facts: BuildFacts = build_facts((plan, project), (policy.is_some(), full_refresh))?;
    let connection = ConsumptionConnection::new(adapter, connection_config);
    let (history, warnings): (ConsumptionHistory, Vec<String>) =
        read_history(py, &connection, (&facts, policy));
    let decision: Option<ChangedDecision> =
        policy.map(|policy| changed_decision(&facts, &history, policy));
    let not_run: BTreeSet<&NodeKey> = decision.iter().flat_map(ChangedDecision::not_run).collect();
    let executions: HashMap<NodeKey, String> = facts
        .runnable
        .iter()
        .filter(|key| !not_run.contains(key))
        .map(|key| (key.clone(), Uuid::now_v7().to_string()))
        .collect();
    let nodes: HashMap<NodeKey, NodeFacts> = facts
        .nodes
        .into_iter()
        .map(|node| (node.key.clone(), node))
        .collect();
    let mut started: BTreeMap<Location, Vec<ConsumptionAppend>> = BTreeMap::new();
    for (key, execution_id) in &executions {
        if let Some(node) = nodes.get(key) {
            started
                .entry(Location::of(&node.relation))
                .or_default()
                .push(started_append(node, execution_id, &run_id));
        }
    }
    let mut failed_start: HashMap<NodeKey, String> = HashMap::new();
    let mut tables: BTreeSet<Location> = BTreeSet::new();
    for (location, rows) in started {
        let written: PyResult<()> = connection
            .ensure_table(py, &location)
            .and_then(|()| connection.append(py, &location, &rows));
        match written {
            Ok(()) => {
                tables.insert(location);
            }
            Err(error) => {
                let message: String = error.to_string();
                for row in rows {
                    failed_start.insert(row.node, message.clone());
                }
            }
        }
    }
    connection.close(py)?;
    let recorder = ConsumptionRecorder {
        connection,
        run_id,
        nodes,
        history,
        executions,
        failed_start,
        base_on_node_complete: NodeCompleteHook::new(&base_on_node_complete),
        recording: Mutex::new(Recording {
            tables,
            ..Recording::default()
        }),
    };
    Ok(Prepared {
        recorder: Bound::new(py, recorder)?,
        decision,
        warnings,
    })
}

/// Recent executions the build reads, with older ones read back when deciding needs them.
pub(crate) fn read_history(
    py: Python<'_>,
    connection: &ConsumptionConnection,
    (facts, policy): (&BuildFacts, Option<&ChangedPolicy>),
) -> (ConsumptionHistory, Vec<String>) {
    let mut warnings: Vec<String> = Vec::new();
    let mut rows: Vec<ConsumptionRow> = Vec::new();
    for location in read_locations(facts, policy.is_some()) {
        match connection.read_recent(py, &location) {
            Ok(read) => rows.extend(read),
            Err(error) => warnings.push(format!(
                "could not read consumed inputs in schema {}: {error}; its models and seeds count as changed",
                location.schema
            )),
        }
    }
    let history: ConsumptionHistory = consumption_history(&rows, RECENT_EXECUTIONS);
    if policy.is_some_and(needs_read_back) {
        let (history, read_back): (ConsumptionHistory, Vec<String>) =
            with_read_back(py, connection, history, &facts.nodes);
        warnings.extend(read_back);
        return (history, warnings);
    }
    (history, warnings)
}

/// What `--changed` does with each runnable node.
pub(crate) fn changed_decision(
    facts: &BuildFacts,
    history: &ConsumptionHistory,
    policy: &ChangedPolicy,
) -> ChangedDecision {
    let mut reasons: Vec<(NodeKey, RunReason)> = Vec::new();
    let mut skipped: Vec<NodeKey> = Vec::new();
    let mut held_back: Vec<(NodeKey, HeldBack)> = Vec::new();
    for (key, outcome) in changed_run_set(&facts.nodes, history, policy) {
        if !facts.runnable.contains(&key) {
            continue;
        }
        match outcome {
            ChangedOutcome::Runs(reason) => reasons.push((key, reason)),
            ChangedOutcome::Unchanged => skipped.push(key),
            ChangedOutcome::HeldBack(why) => held_back.push((key, why)),
        }
    }
    ChangedDecision {
        policy: policy.clone(),
        run: reasons.len(),
        skipped,
        held_back,
        reasons,
    }
}

/// Whether deciding needs older executions: to tell definition from data, or to attribute a change.
fn needs_read_back(policy: &ChangedPolicy) -> bool {
    policy.counts.len() == 1 || matches!(policy.reach, Reach::Job(_))
}

/// `history` plus every older upstream execution its builds read, followed back until none is left.
fn with_read_back(
    py: Python<'_>,
    connection: &ConsumptionConnection,
    history: ConsumptionHistory,
    nodes: &[NodeFacts],
) -> (ConsumptionHistory, Vec<String>) {
    let mut warnings: BTreeSet<String> = BTreeSet::new();
    let history: ConsumptionHistory = read_back_history(history, nodes, |references| {
        let (rows, failures) = read_references(py, connection, references);
        warnings.extend(failures);
        rows
    });
    (history, warnings.into_iter().collect())
}

/// The rows of these executions; a schema that cannot be read leaves its rebuilds counted.
fn read_references(
    py: Python<'_>,
    connection: &ConsumptionConnection,
    references: &[(RelationKey, String)],
) -> (Vec<ConsumptionRow>, Vec<String>) {
    let mut by_location: BTreeMap<Location, Vec<&str>> = BTreeMap::new();
    for (relation, id) in references {
        by_location
            .entry(Location::of(relation))
            .or_default()
            .push(id.as_str());
    }
    let mut rows: Vec<ConsumptionRow> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();
    for (location, ids) in by_location {
        match connection.read_executions(py, &location, &ids) {
            Ok(read) => rows.extend(read),
            Err(error) => warnings.push(format!(
                "could not read earlier executions in schema {}: {error}; rebuilt upstreams there count as changed",
                location.schema
            )),
        }
    }
    (rows, warnings)
}
