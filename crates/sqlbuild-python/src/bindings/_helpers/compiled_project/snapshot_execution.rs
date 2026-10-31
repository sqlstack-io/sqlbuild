//! Frozen selection, executor callbacks and completion belong to one verified worker build.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use pyo3::exceptions::{PyKeyboardInterrupt, PyRuntimeError, PyValueError};
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::PyStringMethods;
use sqlbuild_core::models::ResourceKind;
use sqlbuild_orchestration::runs::models::{NodeOutcome, RunChange, RunExecution, RunOutcome};

use crate::bindings::_helpers::compiled_project::constants::{NO_CHANGE_REASON, SKIP_ACTION};
use crate::bindings::_helpers::compiled_project::snapshot_worker::NativeOrchestrationWorker;
use crate::bindings::_helpers::records::fields::field;

/// This transient owner records only acknowledged outcomes; durable lifecycle remains on the API.
#[derive(Debug)]
pub(crate) struct WorkerBuild {
    pub(crate) worker: Py<NativeOrchestrationWorker>,
    execution: RunExecution,
    completed: Mutex<BTreeMap<String, NodeOutcome>>,
}

impl WorkerBuild {
    pub(crate) fn new(worker: &Bound<'_, NativeOrchestrationWorker>) -> PyResult<Arc<Self>> {
        let state = Arc::new(Self {
            execution: worker.get().execution(worker.py())?,
            worker: worker.clone().unbind(),
            completed: Mutex::default(),
        });
        for key in state.execution.skipped_nodes.keys() {
            state.complete(worker.py(), key, (NodeOutcome::Skipped, None, None))?;
        }
        Ok(state)
    }

    pub(crate) fn has_work(&self) -> bool {
        self.execution
            .claim
            .request
            .selection
            .iter()
            .any(|key| !self.execution.skipped_nodes.contains_key(key))
    }

    pub(crate) fn check_cancelled(&self, py: Python<'_>) -> PyResult<()> {
        if self.worker.bind(py).get().cancel_requested(py) {
            return Err(PyKeyboardInterrupt::new_err("run cancellation requested"));
        }
        Ok(())
    }

    /// Reject an implicitly expanded write set before any executor connection or schema change.
    pub(crate) fn enforce_plan_writers(
        &self,
        pipeline: &Bound<'_, PyAny>,
    ) -> Result<(), pyo3::PyErr> {
        self.check_cancelled(pipeline.py())?;
        let plan = field(pipeline, "plan_output")?;
        for collection in [
            "model_entries",
            "seed_entries",
            "function_entries",
            "source_load_entries",
        ] {
            for entry in field(&plan, collection)?.try_iter()? {
                let entry = entry?;
                let identity = field(&entry, "key")?;
                let kind = field(&identity, "resource_type")?
                    .str()?
                    .to_str()?
                    .to_owned();
                let key = format!("{kind}:{}", field(&identity, "name")?.str()?);
                if !self.execution.claim.request.write_set.contains(&key) {
                    return Err(PyValueError::new_err(format!(
                        "planned writer {key} is outside the frozen run write set"
                    )));
                }
                if kind == ResourceKind::Model.as_str()
                    && field(&entry, "action")?.str()?.to_str()? == SKIP_ACTION
                {
                    let outcome = if field(&entry, "reason")?.str()?.to_str()? == NO_CHANGE_REASON {
                        NodeOutcome::Success
                    } else {
                        NodeOutcome::Skipped
                    };
                    self.complete(pipeline.py(), &key, (outcome, None, None))?;
                }
            }
        }
        Ok(())
    }

    fn complete(
        &self,
        py: Python<'_>,
        key: &str,
        (outcome, rows_affected, error_code): (NodeOutcome, Option<i64>, Option<String>),
    ) -> PyResult<()> {
        if !self.execution.claim.request.selection.contains(key) {
            return Ok(());
        }
        if self
            .completed
            .lock()
            .map_err(|_| poisoned())?
            .contains_key(key)
        {
            return Ok(());
        }
        self.worker.bind(py).get().change(
            py,
            &format!("node-finished/{key}"),
            RunChange::NodeFinished {
                node_key: key.into(),
                outcome,
                rows_affected,
                error_code,
                skipped_because_of: self.execution.skipped_nodes.get(key).cloned(),
            },
        )?;
        self.completed
            .lock()
            .map_err(|_| poisoned())?
            .insert(key.into(), outcome);
        Ok(())
    }

    pub(crate) fn start(
        &self,
        py: Python<'_>,
        name: &str,
        resource_kind: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        self.check_cancelled(py)?;
        let kind = field(resource_kind, "value")?.extract::<String>()?;
        let kind = match kind.as_str() {
            "table" | "view" | "snapshot" | "custom" => "model",
            kind => kind,
        };
        let key = format!("{kind}:{name}");
        if self.execution.claim.request.selection.contains(&key) {
            self.worker.bind(py).get().change(
                py,
                &format!("node-started/{key}"),
                RunChange::NodeStarted { node_key: key },
            )?;
        }
        Ok(())
    }

    /// Existing native progress callbacks publish selected-node evidence without Python relays.
    pub(crate) fn completed(&self, result: &Bound<'_, PyAny>, key: &str) -> PyResult<()> {
        if self.execution.claim.request.selection.contains(key) {
            let status: String = field(result, "status")?.str()?.extract()?;
            let outcome = match status.as_str() {
                "success" => {
                    if result.hasattr("warning_messages")?
                        && field(result, "warning_messages")?.is_truthy()?
                    {
                        NodeOutcome::Warn
                    } else {
                        NodeOutcome::Success
                    }
                }
                "failed" => {
                    if self
                        .worker
                        .bind(result.py())
                        .get()
                        .refresh_cancellation(result.py())?
                    {
                        NodeOutcome::Cancelled
                    } else {
                        NodeOutcome::Failed
                    }
                }
                "skipped" => NodeOutcome::Skipped,
                _ => {
                    return Err(PyValueError::new_err(format!(
                        "unsupported worker node outcome {status}"
                    )));
                }
            };
            let rows = match result.getattr_opt("rows_affected")? {
                Some(value) => value.extract()?,
                None => None,
            };
            let error = match result.getattr_opt("error_code")? {
                Some(value) => value.extract()?,
                None => None,
            };
            self.complete(result.py(), key, (outcome, rows, error))?;
        }
        Ok(())
    }

    /// Never call missing selected nodes successful merely because the executor returned zero.
    pub(crate) fn outcome(&self, py: Python<'_>, exit_code: i32) -> PyResult<RunOutcome> {
        let cancelled = self.worker.bind(py).get().refresh_cancellation(py)?;
        for key in &self.execution.claim.request.selection {
            self.complete(
                py,
                key,
                (
                    if cancelled {
                        NodeOutcome::Cancelled
                    } else {
                        NodeOutcome::Skipped
                    },
                    None,
                    None,
                ),
            )?;
        }
        let outcome = {
            let completed = self.completed.lock().map_err(|_| poisoned())?;
            let good = completed
                .values()
                .any(|value| matches!(value, NodeOutcome::Success | NodeOutcome::Warn));
            let bad = completed.values().any(|value| {
                matches!(
                    value,
                    NodeOutcome::Failed | NodeOutcome::Skipped | NodeOutcome::Cancelled
                )
            });
            if cancelled {
                RunOutcome::Cancelled
            } else if bad && good {
                RunOutcome::Partial
            } else if bad || exit_code != 0 {
                RunOutcome::Failed
            } else if completed.values().any(|value| *value == NodeOutcome::Warn) {
                RunOutcome::Warn
            } else {
                RunOutcome::Success
            }
        };
        Ok(outcome)
    }

    /// Terminal publication follows executor teardown and the invocation's bounded Recall drain.
    pub(crate) fn finish(&self, py: Python<'_>, exit_code: i32) -> PyResult<RunOutcome> {
        let outcome = self.outcome(py, exit_code)?;
        self.worker.bind(py).get().change(
            py,
            "worker-finished",
            RunChange::Finished {
                outcome,
                reason: "worker executor completed and closed its connections".into(),
                exit_code: Some(i64::from(exit_code)),
            },
        )?;
        Ok(outcome)
    }
}

fn poisoned() -> pyo3::PyErr {
    PyRuntimeError::new_err("worker build outcome lock is unavailable")
}
