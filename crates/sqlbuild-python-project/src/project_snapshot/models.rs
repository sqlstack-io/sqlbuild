//! Public records of project snapshot.

use crate::compiled_project::constants::NO_CHANGE_REASON;
use crate::compiled_project::constants::REPORT_ATTEMPTS;
use crate::compiled_project::constants::SKIP_ACTION;
use crate::compiled_project::constants::SQL_TEST_KEY_PREFIX;
use crate::compiled_project::constants::STORE_BUSY_STATUS;
use crate::project_snapshot::_helpers::snapshot::snapshot_execution::Completion;
use crate::project_snapshot::_helpers::snapshot::snapshot_execution::is_function;
use crate::project_snapshot::_helpers::snapshot::snapshot_execution::poisoned;
use crate::project_snapshot::_helpers::snapshot::snapshot_execution::severity;
use crate::project_snapshot::_helpers::snapshot::snapshot_worker::WorkerOwner;
use crate::project_snapshot::_helpers::snapshot::snapshot_worker::unavailable;
use crate::project_snapshot::types::NodeKeys;
use pyo3::exceptions::PyKeyboardInterrupt;
use pyo3::exceptions::PyRuntimeError;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::Bound;
use pyo3::prelude::Py;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyAnyMethods;
use pyo3::prelude::PyRef;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::pyclass;
use pyo3::pymethods;
use pyo3::types::PyStringMethods;
use sqlbuild_core::models::ResourceKind;
use sqlbuild_core::redaction::main::redact_error_text::redact_error_text;
use sqlbuild_host::models::SqlTestOutcome;
use sqlbuild_host::types::ResultEnum;
use sqlbuild_orchestration::api::errors::ClientError;
use sqlbuild_orchestration::runs::models::NodeOutcome;
use sqlbuild_orchestration::runs::models::RunChange;
use sqlbuild_orchestration::runs::models::RunCommand;
use sqlbuild_orchestration::runs::models::RunExecution;
use sqlbuild_orchestration::runs::models::RunOutcome;
use sqlbuild_orchestration::snapshots::main::publication::publish_remote_snapshot::publish_remote_snapshot;
use sqlbuild_orchestration::snapshots::models::SnapshotInput;
use sqlbuild_orchestration::snapshots::models::SnapshotRecord;
use sqlbuild_orchestration::worker::errors::WorkerError;
use sqlbuild_orchestration::worker::main::connect_worker::connect_worker;
use sqlbuild_orchestration::worker::main::start_worker::start_worker;
use sqlbuild_python_base::records::main::fields::field::field;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering;
use uuid::Uuid;

/// A data-only publication; this object never contains adapters, connections or user callables.
#[pyclass(frozen, module = "sqlbuild._native")]
#[derive(Debug)]
pub struct NativeOrchestrationSnapshot {
    pub input: SnapshotInput,
}

impl NativeOrchestrationSnapshot {
    /// Native callers retain transport errors so they can report an uncertain acknowledgement.
    pub fn publication(
        &self,
        py: Python<'_>,
        server_url: &str,
        bearer: &str,
    ) -> PyResult<Result<SnapshotRecord, ClientError>> {
        py.detach(|| {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
            let result = runtime.block_on(publish_remote_snapshot(server_url, bearer, &self.input));
            runtime.shutdown_timeout(std::time::Duration::from_secs(1));
            Ok(result)
        })
    }
}

#[pymethods]
impl NativeOrchestrationSnapshot {
    /// Serialize only at the public boundary; native transports can use the retained input.
    pub(crate) fn to_json(&self, py: Python<'_>) -> PyResult<String> {
        py.detach(|| serde_json::to_string(&self.input))
            .map_err(|error| PyValueError::new_err(error.to_string()))
    }

    /// Publish retained data without projecting it through Python, releasing the GIL during IO.
    pub(crate) fn publish(
        &self,
        py: Python<'_>,
        server_url: &str,
        bearer: &str,
    ) -> PyResult<String> {
        let record = self
            .publication(py, server_url, bearer)?
            .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
        py.detach(|| {
            serde_json::to_string(&record)
                .map_err(|error| PyRuntimeError::new_err(error.to_string()))
        })
    }

    /// Retain the verified claim and its independent native heartbeat in the worker interpreter.
    pub(crate) fn start_worker(
        &self,
        py: Python<'_>,
        server_url: &str,
        bearer: &str,
        identity: (String, String),
    ) -> PyResult<NativeOrchestrationWorker> {
        NativeOrchestrationWorker::start(self, py, (server_url, bearer), identity)
    }
}

/// This transient owner records only acknowledged outcomes; durable lifecycle remains on the API.
#[derive(Debug)]
pub struct WorkerBuild {
    pub worker: Py<NativeOrchestrationWorker>,
    execution: RunExecution,
    completed: Mutex<BTreeMap<String, NodeOutcome>>,
    checks: Mutex<BTreeMap<String, Option<NodeOutcome>>>,
    /// The snapshot's audits and SQL tests, each with the node keys it checks.
    attached: BTreeMap<String, Vec<String>>,
    /// Each SQL test with every node a failure of it blocks in a build: its expected, assertion and reference models and the functions it checks.
    blocked: BTreeMap<String, Vec<String>>,
    /// How many of each function's planned tests have still to report; a function is built before its tests run.
    untested: Mutex<BTreeMap<String, usize>>,
    /// A built function's result, held until its tests report so a failing one can fail it instead.
    held: Mutex<BTreeMap<String, Completion>>,
    /// Each snapshot node's direct upstream node keys, for naming what a skipped node waited on.
    upstream: BTreeMap<String, Vec<String>>,
    /// The failed node each finished node traces back to: itself when it failed, its cause when skipped.
    causes: Mutex<BTreeMap<String, String>>,
    /// The worst attached outcome of each node, for commands that only run tests or audits.
    checked: Mutex<BTreeMap<String, NodeOutcome>>,
    /// Whether the command returned, having run every check it planned, rather than being refused or raising.
    returned: AtomicBool,
    /// Why the worker refused the run before executing anything, reported as the run's status reason.
    refusal: Mutex<Option<String>>,
    /// Error-severity audit failures the build let through, reported before the run finishes.
    ignored_audit_failures: AtomicU32,
}

impl WorkerBuild {
    pub fn new(
        worker: &Bound<'_, NativeOrchestrationWorker>,
        (attached, blocked, upstream): (NodeKeys, NodeKeys, NodeKeys),
    ) -> PyResult<Arc<Self>> {
        let state = Arc::new(Self {
            execution: worker.get().execution(worker.py())?,
            worker: worker.clone().unbind(),
            completed: Mutex::default(),
            checks: Mutex::default(),
            attached,
            blocked,
            untested: Mutex::default(),
            held: Mutex::default(),
            upstream,
            causes: Mutex::default(),
            checked: Mutex::default(),
            returned: AtomicBool::new(false),
            ignored_audit_failures: AtomicU32::new(0),
            refusal: Mutex::default(),
        });
        for key in state.execution.skipped_nodes.keys() {
            state.complete(worker.py(), key, (NodeOutcome::Skipped, None, (None, None)))?;
        }
        Ok(state)
    }

    pub fn command(&self) -> RunCommand {
        self.execution.claim.request.command
    }

    /// The server run this build executes.
    pub fn run_id(&self) -> uuid::Uuid {
        self.execution.claim.request.run_id
    }

    /// The build let `count` error-severity audit failures through.
    pub fn ignored_audit_failures(&self, count: i64) {
        self.ignored_audit_failures.store(
            u32::try_from(count.max(0)).unwrap_or(u32::MAX),
            Ordering::SeqCst,
        );
    }

    /// Test and audit runs execute only attached checks, so their nodes take the checks' outcome.
    fn checks_only(&self) -> bool {
        matches!(self.command(), RunCommand::Test | RunCommand::Audit)
    }

    /// Record that the command returned instead of being refused or raising.
    pub fn command_returned(&self) {
        self.returned.store(true, Ordering::SeqCst);
    }

    pub fn has_work(&self) -> bool {
        self.execution
            .claim
            .request
            .selection
            .iter()
            .any(|key| !self.execution.skipped_nodes.contains_key(key))
    }

    /// Record why the run is refused; the terminal report carries it as the run's status reason.
    pub fn refuse(&self, reason: &str) -> PyResult<()> {
        *self.refusal.lock().map_err(|_| poisoned())? = Some(reason.to_owned());
        Ok(())
    }

    pub fn check_cancelled(&self, py: Python<'_>) -> PyResult<()> {
        if self.worker.bind(py).get().cancel_requested(py) {
            return Err(PyKeyboardInterrupt::new_err("run cancellation requested"));
        }
        Ok(())
    }

    /// Reject writers outside the frozen write set before any executor connection.
    pub fn enforce_writers(&self, py: Python<'_>, keys: &[String]) -> Result<(), pyo3::PyErr> {
        self.check_cancelled(py)?;
        match keys
            .iter()
            .find(|key| !self.execution.claim.request.write_set.contains(*key))
        {
            Some(key) => Err(PyValueError::new_err(format!(
                "planned writer {key} is outside the frozen run write set"
            ))),
            None => Ok(()),
        }
    }

    /// Reject an implicitly expanded write set before any executor connection or schema change.
    pub fn enforce_plan_writers(&self, pipeline: &Bound<'_, PyAny>) -> Result<(), pyo3::PyErr> {
        self.check_cancelled(pipeline.py())?;
        let plan = field(pipeline, "plan_output")?;
        self.count_planned_tests(&plan)?;
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
                    self.complete(pipeline.py(), &key, (outcome, None, (None, None)))?;
                }
            }
        }
        Ok(())
    }

    /// Count each function's tests this build plans, so a built function waits only for tests that will report.
    fn count_planned_tests(&self, plan: &Bound<'_, PyAny>) -> PyResult<()> {
        let mut untested = self.untested.lock().map_err(|_| poisoned())?;
        for entry in field(plan, "test_entries")?.try_iter()? {
            let test = format!("{SQL_TEST_KEY_PREFIX}{}", field(&entry?, "name")?.str()?);
            for node in self.blocked.get(&test).into_iter().flatten() {
                if is_function(node) {
                    *untested.entry(node.clone()).or_default() += 1;
                }
            }
        }
        Ok(())
    }

    /// Nodes `build --changed` leaves unbuilt are up to date, as a planner `no_change` skip is.
    pub fn unchanged(&self, py: Python<'_>, keys: &[String]) -> PyResult<()> {
        for key in keys {
            self.complete(py, key, (NodeOutcome::Success, None, (None, None)))?;
        }
        Ok(())
    }

    /// Nodes `build --changed` holds back wait for the job that refreshes them, with why.
    pub fn held_back(&self, py: Python<'_>, nodes: &[(String, String)]) -> PyResult<()> {
        for (key, why) in nodes {
            self.complete(
                py,
                key,
                (
                    NodeOutcome::Skipped,
                    None,
                    (None, Some(format!("held back: {why}"))),
                ),
            )?;
        }
        Ok(())
    }

    /// A skip in a build names the failed node upstream of it; a test or audit run that raised skips what it never checked, with no cause.
    fn complete(
        &self,
        py: Python<'_>,
        key: &str,
        (outcome, rows_affected, (error_code, error_message)): (
            NodeOutcome,
            Option<i64>,
            (Option<String>, Option<String>),
        ),
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
        let skipped_because_of = match outcome {
            NodeOutcome::Skipped => match self.execution.skipped_nodes.get(key) {
                Some(cause) => Some(cause.clone()),
                None if !self.checks_only() => self.failed_upstream(key)?,
                None => None,
            },
            _ => None,
        };
        self.worker.bind(py).get().change(
            py,
            &format!("node-finished/{key}"),
            RunChange::NodeFinished {
                node_key: key.into(),
                outcome,
                rows_affected,
                error_code,
                skipped_because_of: skipped_because_of.clone(),
                error_message: error_message.as_deref().map(redact_error_text),
            },
        )?;
        let cause = match outcome {
            NodeOutcome::Failed => Some(key.to_owned()),
            _ => skipped_because_of,
        };
        if let Some(cause) = cause {
            self.causes
                .lock()
                .map_err(|_| poisoned())?
                .insert(key.into(), cause);
        }
        self.completed
            .lock()
            .map_err(|_| poisoned())?
            .insert(key.into(), outcome);
        Ok(())
    }

    pub fn start(
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
        self.start_key(py, &format!("{kind}:{name}"))
    }

    /// Report a selected node as started unless its outcome is final, such as a planner skip.
    pub fn start_key(&self, py: Python<'_>, key: &str) -> PyResult<()> {
        self.check_cancelled(py)?;
        if self.execution.claim.request.selection.contains(key)
            && !self
                .completed
                .lock()
                .map_err(|_| poisoned())?
                .contains_key(key)
        {
            self.worker.bind(py).get().change(
                py,
                &format!("node-started/{key}"),
                RunChange::NodeStarted {
                    node_key: key.into(),
                },
            )?;
        }
        Ok(())
    }

    /// Report a selected node's outcome when the command reports no executor result for it.
    pub fn finish_key(&self, py: Python<'_>, key: &str, outcome: NodeOutcome) -> PyResult<()> {
        self.complete(py, key, (outcome, None, (None, None)))
    }

    /// A test or audit the snapshot records has started; a selected SQL test reports as a node.
    pub fn start_attached(&self, py: Python<'_>, key: &str) -> PyResult<()> {
        self.check_cancelled(py)?;
        let Some(nodes) = self.attached.get(key) else {
            return Ok(());
        };
        if self.execution.claim.request.selection.contains(key) {
            return self.start_key(py, key);
        }
        if self
            .checks
            .lock()
            .map_err(|_| poisoned())?
            .contains_key(key)
        {
            return Ok(());
        }
        self.worker.bind(py).get().change(
            py,
            &format!("check-started/{key}"),
            RunChange::CheckStarted {
                node_key: key.into(),
            },
        )?;
        self.checks
            .lock()
            .map_err(|_| poisoned())?
            .insert(key.into(), None);
        if self.checks_only() {
            for node in nodes {
                self.start_key(py, node)?;
            }
        }
        Ok(())
    }

    /// A test or audit finished; a check the snapshot does not record is not reported.
    pub fn finish_attached(&self, py: Python<'_>, key: &str, outcome: NodeOutcome) -> PyResult<()> {
        let Some(nodes) = self.attached.get(key) else {
            return Ok(());
        };
        if outcome != NodeOutcome::Skipped {
            self.start_attached(py, key)?;
        }
        let outcome = if outcome == NodeOutcome::Failed
            && self.worker.bind(py).get().refresh_cancellation(py)?
        {
            NodeOutcome::Cancelled
        } else {
            outcome
        };
        if self.execution.claim.request.selection.contains(key) {
            self.complete(py, key, (outcome, None, (None, None)))?;
        } else {
            self.finish_check(py, key, outcome)?;
        }
        if outcome == NodeOutcome::Skipped {
            return Ok(());
        }
        let mut checked = self.checked.lock().map_err(|_| poisoned())?;
        for node in nodes {
            let worst = checked.get(node).map_or(outcome, |previous| {
                if severity(outcome) > severity(*previous) {
                    outcome
                } else {
                    *previous
                }
            });
            checked.insert(node.clone(), worst);
        }
        Ok(())
    }

    /// The failed node nearest upstream of `key` in this run, or the cause a skipped one carries.
    fn failed_upstream(&self, key: &str) -> PyResult<Option<String>> {
        let causes = self.causes.lock().map_err(|_| poisoned())?;
        let mut seen: BTreeSet<&str> = BTreeSet::from([key]);
        let mut pending: VecDeque<&str> = VecDeque::from([key]);
        while let Some(node) = pending.pop_front() {
            for upstream in self.upstream.get(node).into_iter().flatten() {
                if let Some(cause) = causes.get(upstream) {
                    return Ok(Some(cause.clone()));
                }
                if seen.insert(upstream) {
                    pending.push_back(upstream);
                }
            }
        }
        Ok(None)
    }

    /// A selected SQL test completes itself; a failing one, found by its compiled name such as `parent [case]`, fails the nodes it blocks; each test that reports releases a function waiting on its tests.
    fn finished_test(&self, result: &Bound<'_, PyAny>) -> PyResult<()> {
        let py = result.py();
        let outcome: String = field(result, "outcome")?.str()?.extract()?;
        let passed = SqlTestOutcome::from_value(&outcome) == Some(SqlTestOutcome::Pass);
        let cancelled = !passed && self.worker.bind(py).get().refresh_cancellation(py)?;
        let name: String = field(result, "test_name")?.extract()?;
        let key = format!("{SQL_TEST_KEY_PREFIX}{name}");
        let error: Option<String> = match result.getattr_opt("error_code")? {
            Some(value) => value.extract()?,
            None => None,
        };
        let message: Option<String> = match result.getattr_opt("error_message")? {
            Some(value) => value.extract()?,
            None => None,
        };
        let node = match (passed, cancelled) {
            (true, _) => NodeOutcome::Success,
            (false, true) => NodeOutcome::Cancelled,
            (false, false) => NodeOutcome::Failed,
        };
        self.complete(py, &key, (node, None, (error.clone(), message.clone())))?;
        if !passed && !cancelled {
            for model in self.blocked.get(&key).into_iter().flatten() {
                self.complete(
                    py,
                    model,
                    (NodeOutcome::Failed, None, (error.clone(), message.clone())),
                )?;
            }
        }
        self.release_tested(py, &key)
    }

    /// Count `test` as reported for each function it checks, and publish a held function once none of its tests is left.
    fn release_tested(&self, py: Python<'_>, test: &str) -> PyResult<()> {
        let mut released: Vec<(String, Completion)> = Vec::new();
        {
            let mut untested = self.untested.lock().map_err(|_| poisoned())?;
            let mut held = self.held.lock().map_err(|_| poisoned())?;
            for node in self.blocked.get(test).into_iter().flatten() {
                let Some(remaining) = untested.get_mut(node) else {
                    continue;
                };
                *remaining = remaining.saturating_sub(1);
                if *remaining == 0
                    && let Some(completion) = held.remove(node)
                {
                    released.push((node.clone(), completion));
                }
            }
        }
        for (node, completion) in released {
            self.complete(py, &node, completion)?;
        }
        Ok(())
    }

    /// Publish a node's result, or hold a built function's while its tests have still to report.
    fn complete_or_hold(&self, py: Python<'_>, key: &str, completion: Completion) -> PyResult<()> {
        let waiting = !self.checks_only()
            && matches!(completion.0, NodeOutcome::Success | NodeOutcome::Warn)
            && self
                .untested
                .lock()
                .map_err(|_| poisoned())?
                .get(key)
                .is_some_and(|remaining| *remaining > 0);
        if waiting {
            self.held
                .lock()
                .map_err(|_| poisoned())?
                .insert(key.into(), completion);
            return Ok(());
        }
        self.complete(py, key, completion)
    }

    /// Existing native progress callbacks publish selected-node evidence without Python relays.
    pub fn completed(&self, result: &Bound<'_, PyAny>, key: &str) -> PyResult<()> {
        if key.starts_with(SQL_TEST_KEY_PREFIX) {
            return self.finished_test(result);
        }
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
            let message = match result.getattr_opt("error_message")? {
                Some(value) => value.extract()?,
                None => None,
            };
            self.complete_or_hold(result.py(), key, (outcome, rows, (error, message)))?;
        }
        Ok(())
    }

    pub fn start_check(&self, py: Python<'_>, name: &str) -> PyResult<()> {
        self.check_cancelled(py)?;
        let key = format!("check:{name}");
        self.worker.bind(py).get().change(
            py,
            &format!("check-started/{key}"),
            RunChange::CheckStarted {
                node_key: key.clone(),
            },
        )?;
        self.checks
            .lock()
            .map_err(|_| poisoned())?
            .insert(key, None);
        Ok(())
    }

    /// A completion without our actual-execution start callback is a blocked check, not a failure.
    pub fn completed_check(&self, result: &Bound<'_, PyAny>) -> PyResult<()> {
        let name: String = field(result, "node_name")?.extract()?;
        let key = format!("check:{name}");
        let started = self
            .checks
            .lock()
            .map_err(|_| poisoned())?
            .contains_key(&key);
        let outcome = if !started {
            NodeOutcome::Skipped
        } else if field(result, "passed")?.is_truthy()? {
            NodeOutcome::Success
        } else if field(result, "warned")?.is_truthy()? {
            NodeOutcome::Warn
        } else if self
            .worker
            .bind(result.py())
            .get()
            .refresh_cancellation(result.py())?
        {
            NodeOutcome::Cancelled
        } else {
            NodeOutcome::Failed
        };
        self.finish_check(result.py(), &key, outcome)
    }

    fn finish_check(&self, py: Python<'_>, key: &str, outcome: NodeOutcome) -> PyResult<()> {
        self.worker.bind(py).get().change(
            py,
            &format!("check-finished/{key}"),
            RunChange::CheckFinished {
                node_key: key.into(),
                outcome,
            },
        )?;
        self.checks
            .lock()
            .map_err(|_| poisoned())?
            .insert(key.into(), Some(outcome));
        Ok(())
    }

    /// Never call unseen nodes successful unless the executor returned; a refused command proves only its failures.
    pub fn outcome(&self, py: Python<'_>, exit_code: i32) -> PyResult<RunOutcome> {
        let cancelled = self.worker.bind(py).get().refresh_cancellation(py)?;
        let checks_only = self.checks_only();
        if checks_only && !cancelled {
            let checked = self.checked.lock().map_err(|_| poisoned())?.clone();
            let returned = self.returned.load(Ordering::SeqCst);
            for key in &self.execution.claim.request.selection {
                let seen = checked.get(key).copied();
                let outcome = if returned {
                    Some(seen.unwrap_or(NodeOutcome::Success))
                } else {
                    seen.filter(|outcome| {
                        matches!(outcome, NodeOutcome::Failed | NodeOutcome::Cancelled)
                    })
                };
                if let Some(outcome) = outcome {
                    self.complete(py, key, (outcome, None, (None, None)))?;
                }
            }
        }
        let held: Vec<(String, Completion)> =
            std::mem::take(&mut *self.held.lock().map_err(|_| poisoned())?)
                .into_iter()
                .collect();
        for (key, completion) in held {
            self.complete(py, &key, completion)?;
        }
        let returned = self.returned.load(Ordering::SeqCst);
        let build = self.command().is_build();
        let source_needs_no_run = |key: &str| build && key.starts_with("source:");
        for key in &self.execution.claim.request.selection {
            let outcome = if cancelled {
                NodeOutcome::Cancelled
            } else if returned && source_needs_no_run(key) {
                NodeOutcome::Success
            } else {
                NodeOutcome::Skipped
            };
            self.complete(py, key, (outcome, None, (None, None)))?;
        }
        let outcome = {
            let completed = self.completed.lock().map_err(|_| poisoned())?;
            let outcomes: Vec<NodeOutcome> = completed.values().copied().collect();
            let good = completed.iter().any(|(key, value)| {
                matches!(value, NodeOutcome::Success | NodeOutcome::Warn)
                    && !source_needs_no_run(key)
            });
            let bad = outcomes.iter().any(|value| {
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
            } else if outcomes.contains(&NodeOutcome::Warn) {
                RunOutcome::Warn
            } else {
                RunOutcome::Success
            }
        };
        Ok(outcome)
    }

    /// Terminal publication follows executor teardown and the invocation's bounded Recall drain.
    pub fn finish(&self, py: Python<'_>, exit_code: i32) -> PyResult<RunOutcome> {
        let outcome = self.outcome(py, exit_code)?;
        let unfinished: Vec<String> = self
            .checks
            .lock()
            .map_err(|_| poisoned())?
            .iter()
            .filter(|(_, outcome)| outcome.is_none())
            .map(|(key, _)| key.clone())
            .collect();
        for key in unfinished {
            self.finish_check(
                py,
                &key,
                if outcome == RunOutcome::Cancelled {
                    NodeOutcome::Cancelled
                } else {
                    NodeOutcome::Skipped
                },
            )?;
        }
        let reason: String = self
            .refusal
            .lock()
            .map_err(|_| poisoned())?
            .clone()
            .unwrap_or_else(|| "worker executor completed and closed its connections".into());
        let ignored: u32 = self.ignored_audit_failures.load(Ordering::SeqCst);
        if ignored > 0 {
            self.worker.bind(py).get().change(
                py,
                "worker-audit-failures-ignored",
                RunChange::AuditFailuresIgnored { count: ignored },
            )?;
        }
        self.worker.bind(py).get().change(
            py,
            "worker-finished",
            RunChange::Finished {
                outcome,
                reason,
                exit_code: Some(i64::from(exit_code)),
            },
        )?;
        Ok(outcome)
    }
}

/// Closing this owner stops and joins heartbeat IO but does not claim warehouse work has stopped.
#[pyclass(frozen, module = "sqlbuild._native")]
#[derive(Debug)]
pub struct NativeOrchestrationWorker {
    pub(crate) owner: Mutex<Option<WorkerOwner>>,
}

impl NativeOrchestrationWorker {
    /// Run `work` on the open owner without holding the GIL; a closed worker is unavailable.
    pub(crate) fn with_owner<T: Send>(
        &self,
        py: Python<'_>,
        work: impl FnOnce(&WorkerOwner) -> PyResult<T> + Send,
    ) -> PyResult<T> {
        py.detach(|| {
            let owner = self.owner.lock().map_err(|_| unavailable())?;
            work(owner.as_ref().ok_or_else(unavailable)?)
        })
    }

    /// SIGTERM may arrive before the periodic heartbeat observes the durable cancellation fact.
    pub fn refresh_cancellation(&self, py: Python<'_>) -> PyResult<bool> {
        self.with_owner(py, |owner| {
            if owner.session.cancel_requested() {
                return Ok(true);
            }
            owner
                .runtime
                .block_on(owner.client.heartbeat())
                .map_err(|error| PyRuntimeError::new_err(error.to_string()))
        })
    }

    /// Native execution reads the retained request, never caller-provided selection or settings.
    pub fn execution(
        &self,
        py: Python<'_>,
    ) -> PyResult<sqlbuild_orchestration::runs::models::RunExecution> {
        self.with_owner(py, |owner| Ok(owner.session.execution.clone()))
    }

    /// A bounded retry keeps the event identity across an uncertain acknowledgement or busy store.
    pub fn change(&self, py: Python<'_>, key: &str, change: RunChange) -> PyResult<()> {
        self.with_owner(py, |owner| {
            let event_id = Uuid::new_v5(&owner.session.execution.claim.claim_id, key.as_bytes());
            owner.runtime.block_on(async {
                let mut attempts = 0;
                loop {
                    attempts += 1;
                    match owner.client.report(event_id, change.clone()).await {
                        Ok(_) => return Ok(()),
                        Err(error)
                            if attempts < REPORT_ATTEMPTS
                                && match &error {
                                    WorkerError::Transport(_) => true,
                                    WorkerError::Rejected(status) => {
                                        status.as_u16() == STORE_BUSY_STATUS
                                    }
                                    _ => false,
                                } =>
                        {
                            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                        }
                        Err(error) => return Err(PyRuntimeError::new_err(error.to_string())),
                    }
                }
            })
        })
    }

    /// Verify this retained baseline before the caller applies run settings or opens a warehouse.
    pub fn start(
        snapshot: &NativeOrchestrationSnapshot,
        py: Python<'_>,
        (server_url, bearer): (&str, &str),
        (run_id, claim_id): (String, String),
    ) -> PyResult<NativeOrchestrationWorker> {
        let identity = (
            Uuid::parse_str(&run_id).map_err(|error| PyValueError::new_err(error.to_string()))?,
            Uuid::parse_str(&claim_id).map_err(|error| PyValueError::new_err(error.to_string()))?,
        );
        py.detach(|| {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
            let connected = runtime.block_on(async {
                let client = connect_worker(server_url, bearer, identity)?;
                let session = start_worker(&client, &snapshot.input).await?;
                Ok::<_, sqlbuild_orchestration::worker::errors::WorkerError>((client, session))
            });
            match connected {
                Ok((client, session)) => Ok(NativeOrchestrationWorker {
                    owner: Mutex::new(Some(WorkerOwner {
                        runtime,
                        client,
                        session,
                    })),
                }),
                Err(error) => {
                    runtime.shutdown_timeout(std::time::Duration::from_secs(1));
                    Err(PyRuntimeError::new_err(error.to_string()))
                }
            }
        })
    }
}

#[pymethods]
impl NativeOrchestrationWorker {
    /// The frozen request supplies selection and settings only after baseline verification.
    pub(crate) fn execution_json(&self, py: Python<'_>) -> PyResult<String> {
        self.with_owner(py, |owner| {
            serde_json::to_string(&owner.session.execution)
                .map_err(|error| PyRuntimeError::new_err(error.to_string()))
        })
    }

    /// Cancellation remains sticky; a closed worker cannot authorize further work.
    pub fn cancel_requested(&self, py: Python<'_>) -> bool {
        py.detach(|| {
            self.owner.lock().map_or(true, |owner| {
                owner
                    .as_ref()
                    .is_none_or(|owner| owner.session.cancel_requested())
            })
        })
    }

    /// Retry an uncertain report with the identical event ID and change, never a new identity.
    pub(crate) fn report(
        &self,
        py: Python<'_>,
        event_id: &str,
        change_json: &str,
    ) -> PyResult<bool> {
        let event_id =
            Uuid::parse_str(event_id).map_err(|error| PyValueError::new_err(error.to_string()))?;
        let change: RunChange = serde_json::from_str(change_json)
            .map_err(|error| PyValueError::new_err(error.to_string()))?;
        self.with_owner(py, |owner| {
            owner
                .runtime
                .block_on(owner.client.report(event_id, change))
                .map_err(|error| PyRuntimeError::new_err(error.to_string()))
        })
    }

    pub(crate) fn __enter__(slf: PyRef<'_, Self>) -> PyResult<PyRef<'_, Self>> {
        if slf.owner.lock().map_err(|_| unavailable())?.is_none() {
            return Err(unavailable());
        }
        Ok(slf)
    }

    pub(crate) fn __exit__(
        &self,
        py: Python<'_>,
        _kind: &pyo3::Bound<'_, pyo3::PyAny>,
        _value: &pyo3::Bound<'_, pyo3::PyAny>,
        _traceback: &pyo3::Bound<'_, pyo3::PyAny>,
    ) -> PyResult<()> {
        self.close(py)
    }

    /// Join the heartbeat without publishing a terminal outcome or releasing physical claims.
    pub(crate) fn close(&self, py: Python<'_>) -> PyResult<()> {
        py.detach(|| {
            let mut owner = self.owner.lock().map_err(|_| unavailable())?;
            if let Some(active) = owner.take() {
                active.close();
            }
            Ok(())
        })
    }
}

impl Drop for NativeOrchestrationWorker {
    /// GC can hold the GIL; heartbeat teardown joins native IO and never needs Python.
    fn drop(&mut self) {
        if let Ok(owner) = self.owner.get_mut()
            && let Some(owner) = owner.take()
        {
            owner.close();
        }
    }
}
