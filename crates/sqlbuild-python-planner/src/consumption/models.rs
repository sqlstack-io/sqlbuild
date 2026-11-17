//! Public records of consumption.

use crate::consumption::_helpers::facts::MODEL;
use crate::consumption::_helpers::facts::SEED;
use crate::consumption::_helpers::facts::enum_text;
use crate::consumption::_helpers::recorder::ERROR_OUTCOME;
use crate::consumption::_helpers::recorder::FLUSH_EXECUTIONS;
use crate::consumption::_helpers::recorder::FLUSH_INTERVAL;
use crate::consumption::_helpers::recorder::Recording;
use crate::consumption::_helpers::recorder::SUCCESS;
use crate::consumption::_helpers::recorder::kind_of;
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyAnyMethods;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::pyclass;
use pyo3::pymethods;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_front::native_host::models::ConsumptionConnection;
use sqlbuild_python_front::native_host::models::Location;
use sqlbuild_python_front::native_host::models::NodeCompleteHook;
use sqlbuild_state::consumption::main::consumed_inputs::consumed_inputs;
use sqlbuild_state::consumption::main::succeeded_appends::succeeded_appends;
use sqlbuild_state::consumption::models::ChangedPolicy;
use sqlbuild_state::consumption::models::ConsumptionAppend;
use sqlbuild_state::consumption::models::ConsumptionHistory;
use sqlbuild_state::consumption::models::HeldBack;
use sqlbuild_state::consumption::models::NodeFacts;
use sqlbuild_state::consumption::models::NodeKey;
use sqlbuild_state::consumption::models::RunReason;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;

/// Appends `started` rows before a build and `succeeded` rows as its models and seeds complete.
#[pyclass(module = "sqlbuild._native", frozen)]
#[derive(Debug)]
pub struct ConsumptionRecorder {
    pub(crate) connection: ConsumptionConnection,
    pub(crate) run_id: String,
    pub(crate) nodes: HashMap<NodeKey, NodeFacts>,
    pub(crate) history: ConsumptionHistory,
    pub(crate) executions: HashMap<NodeKey, String>,
    pub(crate) failed_start: HashMap<NodeKey, String>,
    pub(crate) base_on_node_complete: NodeCompleteHook,
    pub(crate) recording: Mutex<Recording>,
}

/// What `--changed` decided, for the build's report.
#[derive(Debug)]
pub struct ChangedDecision {
    pub policy: ChangedPolicy,
    pub run: usize,
    /// Unchanged nodes, which are up to date.
    pub skipped: Vec<NodeKey>,
    /// Nodes downstream of a change that wait for a later run.
    pub held_back: Vec<(NodeKey, HeldBack)>,
    pub reasons: Vec<(NodeKey, RunReason)>,
}

impl ChangedDecision {
    /// Every selected node `--changed` leaves unbuilt: unchanged and held back.
    pub fn not_run(&self) -> impl Iterator<Item = &NodeKey> {
        self.skipped
            .iter()
            .chain(self.held_back.iter().map(|(key, _)| key))
    }
}

impl ConsumptionRecorder {
    pub(crate) fn lock(&self) -> PyResult<std::sync::MutexGuard<'_, Recording>> {
        self.recording
            .lock()
            .map_err(|_| PyRuntimeError::new_err("consumption recorder state is poisoned"))
    }

    /// The succeeded rows of one execution that wrote its relation, with their schema.
    pub(crate) fn succeeded(&self, key: &NodeKey) -> Option<(Location, Vec<ConsumptionAppend>)> {
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
    pub(crate) fn queued(&self, mut recording: Recording, key: &NodeKey) -> Recording {
        if let Some((location, rows)) = self.succeeded(key) {
            recording.pending.entry(location).or_default().extend(rows);
            recording.pending_executions += 1;
        }
        recording
    }

    /// The recording after appending every pending row; a failure is reported, and its executions run again.
    pub(crate) fn flushed(&self, py: Python<'_>, mut recording: Recording) -> Recording {
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
    pub fn finish(&self, py: Python<'_>, build_result: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
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
    pub(crate) fn on_node_complete(
        &self,
        py: Python<'_>,
        node_result: Bound<'_, PyAny>,
    ) -> PyResult<()> {
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
    pub(crate) fn before_node_write(
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

/// Everything the build needs from the recorder: the recorder itself and what `--changed` skipped.
#[derive(Debug)]
pub struct Prepared<'py> {
    pub recorder: Bound<'py, ConsumptionRecorder>,
    pub decision: Option<ChangedDecision>,
    pub warnings: Vec<String>,
}
