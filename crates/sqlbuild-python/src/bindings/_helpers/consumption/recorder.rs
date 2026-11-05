//! Record what each model and seed execution consumed, and decide `--changed` skips, around one build.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::{pyclass, pymethods};
use sqlbuild_state::consumption::constants::RECENT_EXECUTIONS;
use sqlbuild_state::consumption::main::changed_run_set::changed_run_set;
use sqlbuild_state::consumption::main::consumed_inputs::consumed_inputs;
use sqlbuild_state::consumption::main::consumption_history::consumption_history;
use sqlbuild_state::consumption::main::started_append::started_append;
use sqlbuild_state::consumption::main::succeeded_appends::succeeded_appends;
use sqlbuild_state::consumption::models::{
    ConsumptionAppend, ConsumptionHistory, ConsumptionRow, ForcedRun, InputFact, NodeFacts,
    NodeKey, RunReason,
};
use uuid::Uuid;

use crate::bindings::_helpers::consumption::facts::{
    BuildFacts, MODEL, SEED, build_facts, enum_text,
};
use crate::bindings::_helpers::native_host::consumption_state::{
    ConsumptionConnection, Location, NodeCompleteHook,
};
use crate::bindings::_helpers::records::fields::field as native_field;

/// Succeeded rows are flushed once this many executions are pending.
const FLUSH_EXECUTIONS: usize = 100;
/// Or once this long has passed since the last flush.
const FLUSH_INTERVAL: Duration = Duration::from_secs(30);
const MODEL_RESULT: &str = "model_name";
const SEED_RESULT: &str = "seed_name";
const SUCCESS: &str = "success";
const ERROR_OUTCOME: &str = "error";

/// The mutable side of one build's recording.
struct Recording {
    pending: BTreeMap<Location, Vec<ConsumptionAppend>>,
    pending_executions: usize,
    pending_seeds: Vec<NodeKey>,
    tables: BTreeSet<Location>,
    last_flush: Instant,
    warnings: Vec<String>,
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

/// Appends `started` rows before a build and `succeeded` rows as its models and seeds complete.
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct ConsumptionRecorder {
    connection: ConsumptionConnection,
    run_id: String,
    nodes: HashMap<NodeKey, NodeFacts>,
    history: ConsumptionHistory,
    executions: HashMap<NodeKey, String>,
    failed_start: HashMap<NodeKey, String>,
    base_on_node_complete: NodeCompleteHook,
    recording: Mutex<Recording>,
}

/// What `--changed` decided, for the build's report.
pub(crate) struct ChangedDecision {
    pub(crate) run: usize,
    pub(crate) skipped: Vec<NodeKey>,
    pub(crate) reasons: Vec<(NodeKey, RunReason)>,
}

fn kind_of(result: &Bound<'_, PyAny>) -> PyResult<Option<(&'static str, String)>> {
    for (field, kind) in [(MODEL_RESULT, MODEL), (SEED_RESULT, SEED)] {
        if let Some(name) = result.getattr_opt(field)? {
            return Ok(Some((kind, name.extract()?)));
        }
    }
    Ok(None)
}

impl ConsumptionRecorder {
    fn lock(&self) -> PyResult<std::sync::MutexGuard<'_, Recording>> {
        self.recording
            .lock()
            .map_err(|_| PyRuntimeError::new_err("consumption recorder state is poisoned"))
    }

    /// The succeeded rows of one execution that wrote its relation, with their schema.
    fn succeeded(&self, key: &NodeKey) -> Option<(Location, Vec<ConsumptionAppend>)> {
        let (Some(node), Some(execution_id)) = (self.nodes.get(key), self.executions.get(key))
        else {
            return None;
        };
        let inputs = consumed_inputs(node, (&self.nodes, &self.history), &self.executions);
        Some((
            Location::of(&node.relation),
            succeeded_appends(node, (execution_id, &self.run_id), &inputs),
        ))
    }

    /// The recording with `key`'s succeeded rows queued.
    fn queued(&self, mut recording: Recording, key: &NodeKey) -> Recording {
        if let Some((location, rows)) = self.succeeded(key) {
            recording.pending.entry(location).or_default().extend(rows);
            recording.pending_executions += 1;
        }
        recording
    }

    /// The recording after appending every pending row; a failure is reported, and its executions run again.
    fn flushed(&self, py: Python<'_>, mut recording: Recording) -> Recording {
        let pending = std::mem::take(&mut recording.pending);
        let wrote: bool = !pending.is_empty();
        for (location, rows) in pending {
            let created: bool = !recording.tables.insert(location.clone());
            let written: PyResult<()> = (if created {
                Ok(())
            } else {
                self.connection.ensure_table(py, &location)
            })
            .and_then(|()| self.connection.append(py, &location, &rows));
            if let Err(error) = written {
                recording.warnings.push(format!(
                    "could not record consumed inputs in schema {}: {error}; those models and seeds run again under --changed",
                    location.schema
                ));
            }
        }
        if wrote && let Err(error) = self.connection.close(py) {
            recording.warnings.push(format!(
                "could not close the consumed-input connection: {error}"
            ));
        }
        recording.pending_executions = 0;
        recording.last_flush = Instant::now();
        recording
    }

    /// Queue the seeds whose error audits passed and flush everything.
    pub(crate) fn finish(
        &self,
        py: Python<'_>,
        build_result: &Bound<'_, PyAny>,
    ) -> PyResult<Vec<String>> {
        let mut failed_audits: BTreeSet<String> = BTreeSet::new();
        for audit in native_field(build_result, "source_audit_results")?.try_iter()? {
            let audit: Bound<'_, PyAny> = audit?;
            let target: Bound<'_, PyAny> = native_field(&audit, "attached_target_name")?;
            if !target.is_none()
                && enum_text(&native_field(&audit, "outcome")?)?.eq_ignore_ascii_case(ERROR_OUTCOME)
            {
                failed_audits.insert(target.extract()?);
            }
        }
        let mut recording = self.lock()?;
        for seed in std::mem::take(&mut recording.pending_seeds) {
            if !failed_audits.contains(&seed.name) {
                *recording = self.queued(std::mem::take(&mut *recording), &seed);
            }
        }
        *recording = self.flushed(py, std::mem::take(&mut *recording));
        Ok(std::mem::take(&mut recording.warnings))
    }
}

#[pymethods]
impl ConsumptionRecorder {
    /// Forward a finished node, then queue a successful model, or a seed until its audits are known.
    fn on_node_complete(&self, py: Python<'_>, node_result: Bound<'_, PyAny>) -> PyResult<()> {
        self.base_on_node_complete.forward(&node_result)?;
        let Some((kind, name)) = kind_of(&node_result)? else {
            return Ok(());
        };
        if !enum_text(&native_field(&node_result, "status")?)?.eq_ignore_ascii_case(SUCCESS) {
            return Ok(());
        }
        let key = NodeKey {
            kind: kind.to_owned(),
            name,
        };
        let mut recording = self.lock()?;
        if kind == SEED {
            recording.pending_seeds.push(key);
            return Ok(());
        }
        *recording = self.queued(std::mem::take(&mut *recording), &key);
        if recording.pending_executions >= FLUSH_EXECUTIONS
            || recording.last_flush.elapsed() >= FLUSH_INTERVAL
        {
            *recording = self.flushed(py, std::mem::take(&mut *recording));
        }
        Ok(())
    }

    /// Fail a model or seed whose `started` row could not be appended, before it writes.
    #[pyo3(signature = (*, entry, connection))]
    fn before_node_write(
        &self,
        entry: Bound<'_, PyAny>,
        connection: Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let _ = connection;
        let name: String = native_field(&entry, "name")?.extract()?;
        let kind: &str = if entry.hasattr("csv_settings")? {
            SEED
        } else {
            MODEL
        };
        let key = NodeKey {
            kind: kind.to_owned(),
            name,
        };
        match self.failed_start.get(&key) {
            Some(error) => Err(PyRuntimeError::new_err(format!(
                "not written: SQLBuild could not record that this {kind} started ({error}), so a crash could not be detected later"
            ))),
            None => Ok(()),
        }
    }
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

/// Everything the build needs from the recorder: the recorder itself and what `--changed` skipped.
pub(crate) struct Prepared<'py> {
    pub(crate) recorder: Bound<'py, ConsumptionRecorder>,
    pub(crate) decision: Option<ChangedDecision>,
    pub(crate) warnings: Vec<String>,
}

/// Read records, decide `--changed` skips, mint execution IDs and append `started` rows.
pub(crate) fn prepare<'py>(
    (plan, project): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    (adapter, connection_config): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    (changed, full_refresh): (bool, bool),
    (run_id, base_on_node_complete): (String, Bound<'py, PyAny>),
) -> PyResult<Prepared<'py>> {
    let py: Python<'py> = plan.py();
    let facts: BuildFacts = build_facts((plan, project), (changed, full_refresh))?;
    let connection = ConsumptionConnection::new(adapter, connection_config);
    let mut warnings: Vec<String> = Vec::new();
    let mut rows: Vec<ConsumptionRow> = Vec::new();
    for location in read_locations(&facts, changed) {
        match connection.read_recent(py, &location) {
            Ok(read) => rows.extend(read),
            Err(error) => warnings.push(format!(
                "could not read consumed inputs in schema {}: {error}; its models and seeds count as changed",
                location.schema
            )),
        }
    }
    let history: ConsumptionHistory = consumption_history(&rows, RECENT_EXECUTIONS);
    let mut skipped: BTreeSet<NodeKey> = BTreeSet::new();
    let decision: Option<ChangedDecision> = if changed {
        let mut reasons: Vec<(NodeKey, RunReason)> = Vec::new();
        for (key, reason) in changed_run_set(&facts.nodes, &history) {
            if !facts.runnable.contains(&key) {
                continue;
            }
            match reason {
                Some(reason) => reasons.push((key, reason)),
                None => {
                    skipped.insert(key);
                }
            }
        }
        Some(ChangedDecision {
            run: reasons.len(),
            skipped: skipped.iter().cloned().collect(),
            reasons,
        })
    } else {
        None
    };
    let executions: HashMap<NodeKey, String> = facts
        .runnable
        .iter()
        .filter(|key| !skipped.contains(*key))
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

/// Why `--changed` runs a node, as one line for verbose output.
pub(crate) fn run_reason_text(reason: &RunReason) -> String {
    match reason {
        RunReason::Forced(ForcedRun::PlannerChange(kind)) => format!("planner change ({kind})"),
        RunReason::Forced(ForcedRun::FullRefresh) => "full refresh".to_owned(),
        RunReason::Forced(ForcedRun::RelationMissing) => "relation missing".to_owned(),
        RunReason::Forced(ForcedRun::CursorInterval) => "cursor interval requested".to_owned(),
        RunReason::Forced(ForcedRun::InputsNotEnumerable) => {
            "inputs cannot be enumerated".to_owned()
        }
        RunReason::NoRecord => "no completed record".to_owned(),
        RunReason::Unfinished => "last execution did not finish".to_owned(),
        RunReason::AmbiguousOrder => "record order is ambiguous".to_owned(),
        RunReason::DefinitionChanged => "definition changed".to_owned(),
        RunReason::InputsChanged => "inputs changed".to_owned(),
        RunReason::InputChanged(input) => format!("input {input} changed"),
        RunReason::InputUnknown(input) => format!("input {input} has no known version"),
        RunReason::UpstreamRuns(key) => format!("upstream {} {} runs", key.kind, key.name),
    }
}
