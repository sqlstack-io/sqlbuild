//! Record what each model and seed execution consumed, and decide `--changed` skips, around one build.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python};
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

use crate::bindings::_helpers::consumption::facts::{BuildFacts, build_facts};
use crate::bindings::_helpers::consumption::state_io::{
    Location, append, ensure_table, read_recent,
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

/// Appends `started` rows before a build and `succeeded` rows as its models and seeds complete.
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct ConsumptionRecorder {
    adapter: Py<PyAny>,
    connection: Py<PyAny>,
    run_id: String,
    nodes: HashMap<NodeKey, NodeFacts>,
    history: ConsumptionHistory,
    executions: HashMap<NodeKey, String>,
    failed_start: HashMap<NodeKey, String>,
    base_on_node_complete: Py<PyAny>,
    recording: Mutex<Recording>,
}

/// What `--changed` decided, for the build's report.
pub(crate) struct ChangedDecision {
    pub(crate) run: usize,
    pub(crate) skipped: Vec<NodeKey>,
    pub(crate) reasons: Vec<(NodeKey, RunReason)>,
}

fn kind_of(result: &Bound<'_, PyAny>) -> PyResult<Option<(&'static str, String)>> {
    for (field, kind) in [(MODEL_RESULT, "model"), (SEED_RESULT, "seed")] {
        if let Some(name) = result.getattr_opt(field)? {
            return Ok(Some((kind, name.extract()?)));
        }
    }
    Ok(None)
}

fn status_text(value: &Bound<'_, PyAny>) -> PyResult<String> {
    let value: Bound<'_, PyAny> = value.getattr_opt("value")?.unwrap_or_else(|| value.clone());
    Ok(value.str()?.extract::<String>()?.to_ascii_lowercase())
}

impl ConsumptionRecorder {
    fn lock(&self) -> PyResult<std::sync::MutexGuard<'_, Recording>> {
        self.recording
            .lock()
            .map_err(|_| PyRuntimeError::new_err("consumption recorder state is poisoned"))
    }

    /// Queue the succeeded rows of one execution that wrote its relation.
    fn queue(&self, recording: &mut Recording, key: &NodeKey) {
        let (Some(node), Some(execution_id)) = (self.nodes.get(key), self.executions.get(key))
        else {
            return;
        };
        let inputs = consumed_inputs(node, (&self.nodes, &self.history), &self.executions);
        recording
            .pending
            .entry(Location::of(&node.relation))
            .or_default()
            .extend(succeeded_appends(
                node,
                (execution_id, &self.run_id),
                &inputs,
            ));
        recording.pending_executions += 1;
    }

    /// Append every pending row; a failure is reported once, and its executions simply run again.
    fn flush(&self, py: Python<'_>, recording: &mut Recording) {
        let adapter: &Bound<'_, PyAny> = self.adapter.bind(py);
        let connection: &Bound<'_, PyAny> = self.connection.bind(py);
        for (location, rows) in std::mem::take(&mut recording.pending) {
            let written: PyResult<()> = (|| {
                if recording.tables.insert(location.clone()) {
                    ensure_table(adapter, connection, &location)?;
                }
                append(adapter, connection, &location, &rows)
            })();
            if let Err(error) = written {
                recording.warnings.push(format!(
                    "could not record consumed inputs in schema {}: {error}; those models and seeds run again under --changed",
                    location.schema
                ));
            }
        }
        recording.pending_executions = 0;
        recording.last_flush = Instant::now();
    }

    /// Queue the seeds whose error audits passed, flush everything and close the connection.
    pub(crate) fn finish(
        &self,
        py: Python<'_>,
        build_result: &Bound<'_, PyAny>,
    ) -> PyResult<Vec<String>> {
        let mut failed_audits: BTreeSet<String> = BTreeSet::new();
        for audit in native_field(build_result, "source_audit_results")?.try_iter()? {
            let audit: Bound<'_, PyAny> = audit?;
            let target: Bound<'_, PyAny> = native_field(&audit, "attached_target_name")?;
            if !target.is_none() && status_text(&native_field(&audit, "outcome")?)? == ERROR_OUTCOME
            {
                failed_audits.insert(target.extract()?);
            }
        }
        let mut recording = self.lock()?;
        for seed in std::mem::take(&mut recording.pending_seeds) {
            if !failed_audits.contains(&seed.name) {
                self.queue(&mut recording, &seed);
            }
        }
        self.flush(py, &mut recording);
        let adapter: &Bound<'_, PyAny> = self.adapter.bind(py);
        let closed = adapter.call_method1("close", (self.connection.bind(py),));
        let warnings: Vec<String> = std::mem::take(&mut recording.warnings);
        closed?;
        Ok(warnings)
    }
}

#[pymethods]
impl ConsumptionRecorder {
    /// Forward a finished node, then queue a successful model, or a seed until its audits are known.
    fn on_node_complete(&self, py: Python<'_>, node_result: Bound<'_, PyAny>) -> PyResult<()> {
        self.base_on_node_complete.bind(py).call1((&node_result,))?;
        let Some((kind, name)) = kind_of(&node_result)? else {
            return Ok(());
        };
        if status_text(&native_field(&node_result, "status")?)? != SUCCESS {
            return Ok(());
        }
        let key = NodeKey {
            kind: kind.to_owned(),
            name,
        };
        let mut recording = self.lock()?;
        if kind == "seed" {
            recording.pending_seeds.push(key);
            return Ok(());
        }
        self.queue(&mut recording, &key);
        if recording.pending_executions >= FLUSH_EXECUTIONS
            || recording.last_flush.elapsed() >= FLUSH_INTERVAL
        {
            self.flush(py, &mut recording);
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
            "seed"
        } else {
            "model"
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

/// The locations whose records this build reads: all of them under `--changed`, else the
/// relations of unselected upstreams whose last execution the selected nodes record.
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
    let connection: Bound<'py, PyAny> = adapter.call_method1("connect", (connection_config,))?;
    let mut warnings: Vec<String> = Vec::new();
    let mut rows: Vec<ConsumptionRow> = Vec::new();
    for location in read_locations(&facts, changed) {
        match read_recent(adapter, &connection, &location) {
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
        let written: PyResult<()> = ensure_table(adapter, &connection, &location)
            .and_then(|()| append(adapter, &connection, &location, &rows));
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
    let recorder = ConsumptionRecorder {
        adapter: adapter.clone().unbind(),
        connection: connection.unbind(),
        run_id,
        nodes,
        history,
        executions,
        failed_start,
        base_on_node_complete: base_on_node_complete.unbind(),
        recording: Mutex::new(Recording {
            pending: BTreeMap::new(),
            pending_executions: 0,
            pending_seeds: Vec::new(),
            tables,
            last_flush: Instant::now(),
            warnings: Vec::new(),
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
