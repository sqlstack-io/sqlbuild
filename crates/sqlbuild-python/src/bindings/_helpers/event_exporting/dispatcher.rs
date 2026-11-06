//! `EventExporterDispatcher`: bounded priority delivery of lifecycle events on Python daemon threads.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::{
    Bound, Py, PyAny, PyAnyMethods, PyResult, Python, pyclass, pyfunction, pymethods,
};
use pyo3::types::{PyBool, PyDict, PyFloat, PyInt, PyString};
use sqlbuild_core::command_output::constants::COMMAND_OUTPUT_RECORD_TYPE;
use sqlbuild_core::event_export::constants::{
    DEFAULT_EVENT_EXPORT_HEALTH_INTERVAL_SECONDS, DEFAULT_EVENT_EXPORT_INVOCATION_TIMEOUT_SECONDS,
    DEFAULT_EVENT_EXPORT_NOTIFICATION_QUEUE_CAPACITY, DEFAULT_EVENT_EXPORT_QUEUE_CAPACITY,
    DEFAULT_EVENT_EXPORT_SHUTDOWN_TIMEOUT_SECONDS, INVOCATION_TERMINAL_EVENT_TYPES,
    OUTCOME_EVENT_TYPES, OUTCOME_RESERVE_FRACTION,
};
use sqlbuild_core::event_export::main::drop_lifecycle_attempts::drop_lifecycle_attempts;
use sqlbuild_core::event_export::main::export_snapshot::export_snapshot;
use sqlbuild_core::event_export::main::queue_lifecycle_event::queue_lifecycle_event;
use sqlbuild_core::event_export::main::route_lifecycle_event::route_lifecycle_event;
use sqlbuild_core::event_export::models::{
    EventQueue, ExportPolicy, ExportSeverity, ExportSnapshot, ExporterCounts, ExporterFilter,
    QueuedEvent, SnapshotInputs,
};
use sqlbuild_host::entries::models::UserCodeHost;

use crate::bindings::_helpers::boundary::python_calls::{KeywordArguments, call_with, imported};
use crate::bindings::_helpers::event_exporting::constants::{
    DISPATCHER_THREAD_NAME, EXIT_JOIN_SECONDS, HEALTH_THREAD_NAME, IDLE_WAIT_SECONDS,
    JOIN_GRACE_SECONDS, NO_SECONDS, NOTIFIER_THREAD_NAME, OUTPUT_SINK_THREAD_NAME, POLL_SECONDS,
    WORKER_ATTRIBUTE,
};
use crate::bindings::_helpers::event_exporting::models::{
    BoundEventExporter, EventExportSummary, EventExporterFailure,
};
use crate::bindings::_helpers::event_exporting::values::{
    export_policy, input_error, severity_from, state_error,
};
use crate::bindings::_helpers::event_exporting::workers::{DispatcherWorker, Lane, WorkerRole};
use crate::bindings::_helpers::native_host::host;
use crate::bindings::_helpers::native_threads::constants::OUTPUT_SUPPRESSED_ATTRIBUTE;
use crate::bindings::_helpers::native_threads::exit_signal::{
    interpreter_exiting, signal_interpreter_exit,
};
use crate::bindings::_helpers::records::fields::field as native_field;

/// One bound lifecycle-event sink.
pub(crate) struct Exporter {
    pub(crate) name: String,
    pub(crate) function: Py<PyAny>,
    pub(crate) provider_arguments: Py<PyAny>,
}

/// One bound command-output sink and the streams it accepts.
pub(crate) struct OutputSink {
    function: Py<PyAny>,
    provider_arguments: Py<PyAny>,
    streams: Vec<String>,
}

/// A queued event with the policy dimensions its failures report.
pub(crate) struct Queued {
    pub(crate) event: Py<PyAny>,
    pub(crate) kind: String,
    pub(crate) severity: ExportSeverity,
}

/// Delivery state, read and written only while holding the dispatcher's Python condition.
pub(crate) struct State {
    pub(crate) bound: bool,
    pub(crate) exporters: Vec<Arc<Exporter>>,
    pub(crate) filters: Vec<ExporterFilter>,
    pub(crate) counts: Vec<ExporterCounts>,
    output_sinks: Arc<Vec<OutputSink>>,
    /// Calls queued to the single command-output sink thread, once it has started.
    output_lane: Option<Py<PyAny>>,
    pub(crate) queue: EventQueue<Queued>,
    sequence: u64,
    accepting: bool,
    pub(crate) deadline: Option<Instant>,
    /// When only outcome events remain deliverable, ahead of the deadline.
    pub(crate) reserve_at: Option<Instant>,
    pub(crate) reserved: bool,
    /// Each exporter's long-lived delivery thread, once started and until retired.
    pub(crate) lanes: Vec<Option<Lane>>,
    /// The exporter each delivered-but-unreported attempt token belongs to.
    pub(crate) tokens: HashMap<u64, usize>,
    pub(crate) next_token: u64,
    /// Retired lanes, whose threads are joined before finalizers run.
    pub(crate) retired: Vec<Lane>,
    /// Whether a timed-out sink call may still be running.
    pub(crate) abandoned_calls: bool,
    pub(crate) stopping: bool,
    pub(crate) force_stop: bool,
    pub(crate) blocked: HashSet<usize>,
    /// The exporter of each abandoned sink call that may still be running, by attempt token.
    pub(crate) stalled: HashMap<u64, usize>,
    /// Outcome events owed to blocked exporters, delivered once their stalled call returns.
    pub(crate) held: Vec<(usize, Queued)>,
    /// Exporters whose sink call timed out since the last dispatch round, to warn about.
    pub(crate) newly_blocked: Vec<String>,
    /// When the dispatcher last charged running sink calls toward their timeout.
    pub(crate) last_charged: Option<Instant>,
    pub(crate) live_invocations: HashSet<u64>,
    pub(crate) next_invocation: u64,
    dispatcher_running: bool,
    shutdown_started: bool,
    shutdown_complete: bool,
    final_summary: Option<Py<EventExportSummary>>,
    pub(crate) failures: VecDeque<EventExporterFailure>,
    pub(crate) notification_stopping: bool,
    pub(crate) health_stopping: bool,
    pub(crate) pending_periodic: Option<ExportSnapshot>,
    pub(crate) periodic_inflight: Option<[u64; 7]>,
    pub(crate) last_published_periodic: [u64; 7],
    pub(crate) final_notification: Option<Py<EventExportSummary>>,
    pub(crate) final_delivered: bool,
}

/// What the dispatcher and its threads share.
pub(crate) struct Shared {
    condition: Py<PyAny>,
    /// The `SimpleQueue` every delivery thread reports `(token, error)` outcomes to.
    pub(crate) results: Py<PyAny>,
    /// Attempt tokens a delivery thread claimed (`True`) before running, or the dispatcher cancelled (`False`).
    pub(crate) claims: Py<PyDict>,
    state: Mutex<State>,
    pub(crate) invocation_timeout: Duration,
    shutdown_timeout: Duration,
    pub(crate) health_interval: Duration,
    notification_capacity: usize,
    pub(crate) failure_callback: Option<Py<PyAny>>,
    pub(crate) summary_callback: Option<Py<PyAny>>,
    notifications: bool,
}

/// Deliver filtered lifecycle events off execution threads with bounded memory.
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct EventExporterDispatcher {
    shared: Arc<Shared>,
    thread: Py<PyAny>,
    notification_thread: Option<Py<PyAny>>,
    health_thread: Option<Py<PyAny>>,
}

/// The dispatcher's Python condition, entered while alive and exited on drop.
pub(crate) struct Held<'py>(Bound<'py, PyAny>);

/// How one command-output sink call ended.
pub(crate) enum Delivery {
    /// The sink returned; `true` when it raised.
    Delivered(bool),
    /// Interpreter exit began while the sink was still running.
    Abandoned,
}

/// Validated dispatcher limits, in seconds and items.
struct Limits {
    queue_capacity: usize,
    shutdown_timeout: Duration,
    invocation_timeout: Duration,
    notification_capacity: usize,
    health_interval: Duration,
}

impl Drop for Held<'_> {
    fn drop(&mut self) {
        let py = self.0.py();
        drop(
            self.0
                .call_method1("__exit__", (py.None(), py.None(), py.None())),
        );
    }
}

fn seconds(value: f64) -> Duration {
    Duration::try_from_secs_f64(value.max(NO_SECONDS)).unwrap_or(Duration::MAX)
}

fn remaining_seconds(deadline: Instant) -> f64 {
    deadline
        .saturating_duration_since(Instant::now())
        .as_secs_f64()
}

/// End a retired command-output sink thread once it has run every call queued before.
fn retire_output_lane(py: Python<'_>, retired: Option<Py<PyAny>>) -> PyResult<()> {
    if let Some(calls) = retired {
        calls.bind(py).call_method1("put", (py.None(),))?;
    }
    Ok(())
}

/// `thread.join(timeout=seconds)` when the thread is alive.
fn join(thread: &Bound<'_, PyAny>, seconds: f64) -> PyResult<()> {
    if thread.call_method0("is_alive")?.is_truthy()? {
        call_with(
            &native_field(thread, "join")?,
            &[("timeout", PyFloat::new(thread.py(), seconds).into_any())],
        )?;
    }
    Ok(())
}

/// Bound exporters read from `BoundEventExporter` values or objects with the same attributes.
fn read_exporters(
    exporters: &Bound<'_, PyAny>,
) -> PyResult<(Vec<Arc<Exporter>>, Vec<ExporterFilter>)> {
    let mut bound: Vec<Arc<Exporter>> = Vec::new();
    let mut filters: Vec<ExporterFilter> = Vec::new();
    for exporter in exporters.try_iter()? {
        let exporter: Bound<'_, PyAny> = exporter?;
        if let Ok(native) = exporter.downcast::<BoundEventExporter>() {
            let native = native.get();
            filters.push(native.filter.clone());
            bound.push(Arc::new(Exporter {
                name: native.name.clone(),
                function: native.function.clone_ref(exporter.py()),
                provider_arguments: native.provider_arguments.clone_ref(exporter.py()),
            }));
            continue;
        }
        filters.push(ExporterFilter {
            event_kinds: native_field(&exporter, "event_kinds")?
                .try_iter()?
                .map(|kind| kind.and_then(|kind| kind.extract::<String>()))
                .collect::<PyResult<_>>()?,
            min_severity: severity_from(&native_field(&exporter, "min_severity")?)?,
        });
        bound.push(Arc::new(Exporter {
            name: native_field(&exporter, "name")?.extract()?,
            function: native_field(&exporter, "function")?.unbind(),
            provider_arguments: native_field(&exporter, "provider_arguments")?.unbind(),
        }));
    }
    Ok((bound, filters))
}

fn read_output_sinks(sinks: &Bound<'_, PyAny>) -> PyResult<Vec<OutputSink>> {
    let mut bound: Vec<OutputSink> = Vec::new();
    for sink in sinks.try_iter()? {
        let sink: Bound<'_, PyAny> = sink?;
        bound.push(OutputSink {
            function: native_field(&sink, "function")?.unbind(),
            provider_arguments: native_field(&sink, "provider_arguments")?.unbind(),
            streams: native_field(&sink, "streams")?
                .try_iter()?
                .map(|stream| stream.and_then(|stream| stream.extract::<String>()))
                .collect::<PyResult<_>>()?,
        });
    }
    Ok(bound)
}

/// `health_interval_seconds` when it is a positive, finite non-boolean number.
fn health_interval(value: &Bound<'_, PyAny>) -> PyResult<f64> {
    let py: Python<'_> = value.py();
    if value.is_none() {
        return Ok(DEFAULT_EVENT_EXPORT_HEALTH_INTERVAL_SECONDS);
    }
    let invalid = || {
        input_error(
            py,
            "event exporter health_interval_seconds must be positive and finite",
        )
    };
    if value.is_instance_of::<PyBool>()
        || !(value.is_instance_of::<PyInt>() || value.is_instance_of::<PyFloat>())
    {
        return Err(invalid());
    }
    let number: f64 = value.extract()?;
    if !number.is_finite() || number <= NO_SECONDS {
        return Err(invalid());
    }
    Ok(number)
}

/// The constructor's limits, validated in the Python dispatcher's order.
fn limits(options: &KeywordArguments<'_>) -> PyResult<Limits> {
    let integer = |name: &str, default: i64| -> PyResult<i64> {
        let value = options.value(name)?;
        if value.is_none() {
            Ok(default)
        } else {
            value.extract()
        }
    };
    let float = |name: &str, default: f64| -> PyResult<f64> {
        let value = options.value(name)?;
        if value.is_none() {
            Ok(default)
        } else {
            value.extract()
        }
    };
    let py: Python<'_> = options.value("exporters")?.py();
    let queue_capacity: i64 = integer("queue_capacity", DEFAULT_EVENT_EXPORT_QUEUE_CAPACITY)?;
    let shutdown_timeout: f64 = float(
        "shutdown_timeout_seconds",
        DEFAULT_EVENT_EXPORT_SHUTDOWN_TIMEOUT_SECONDS,
    )?;
    let invocation_timeout: f64 = float(
        "invocation_timeout_seconds",
        DEFAULT_EVENT_EXPORT_INVOCATION_TIMEOUT_SECONDS,
    )?;
    let notification_capacity: i64 = integer(
        "notification_queue_capacity",
        DEFAULT_EVENT_EXPORT_NOTIFICATION_QUEUE_CAPACITY,
    )?;
    let capacity_error = || input_error(py, "event exporter queue capacities must be at least 1");
    if notification_capacity < 1 {
        return Err(capacity_error());
    }
    if shutdown_timeout < NO_SECONDS || invocation_timeout <= NO_SECONDS {
        return Err(input_error(py, "event exporter timeouts must be positive"));
    }
    let health: f64 = health_interval(&options.value("health_interval_seconds")?)?;
    if queue_capacity < 1 {
        return Err(capacity_error());
    }
    Ok(Limits {
        queue_capacity: usize::try_from(queue_capacity).unwrap_or(usize::MAX),
        shutdown_timeout: seconds(shutdown_timeout),
        invocation_timeout: seconds(invocation_timeout),
        notification_capacity: usize::try_from(notification_capacity).unwrap_or(usize::MAX),
        health_interval: seconds(health),
    })
}

/// Stop every live dispatcher's threads and join them before finalization ends them inside native code.
#[pyfunction]
pub(crate) fn stop_event_exporter_threads(py: Python<'_>) -> PyResult<()> {
    signal_interpreter_exit();
    let mut threads: Vec<Bound<'_, PyAny>> = Vec::new();
    for thread in imported(py, "threading", "enumerate")?
        .call0()?
        .try_iter()?
    {
        let thread: Bound<'_, PyAny> = thread?;
        let Ok(worker) = native_field(&thread, WORKER_ATTRIBUTE) else {
            continue;
        };
        if let Ok(worker) = worker.downcast::<DispatcherWorker>() {
            worker.get().shared().stop_for_exit(py)?;
            threads.push(thread);
        }
    }
    for thread in threads {
        join(&thread, EXIT_JOIN_SECONDS)?;
    }
    Ok(())
}

impl State {
    /// Count one failed attempt and queue its sanitized diagnostic, returning whether one was queued.
    pub(crate) fn note_failure(
        &mut self,
        diagnostics: Option<usize>,
        (index, kind, severity): (usize, &str, ExportSeverity),
        error_type: String,
    ) -> bool {
        let mut exporter_name: String = String::new();
        if let Some(counts) = self.counts.get_mut(index) {
            counts.failed += 1;
        }
        if let Some(exporter) = self.exporters.get(index) {
            exporter_name.clone_from(&exporter.name);
        }
        if diagnostics.is_none_or(|capacity| self.failures.len() >= capacity) {
            return false;
        }
        self.failures.push_back(EventExporterFailure {
            exporter_name,
            error_type,
            event_kind: kind.to_owned(),
            event_severity: severity,
        });
        true
    }
}

impl Shared {
    pub(crate) fn hold<'py>(&self, py: Python<'py>) -> PyResult<Held<'py>> {
        let condition = self.condition.bind(py).clone();
        condition.call_method0("__enter__")?;
        Ok(Held(condition))
    }

    pub(crate) fn locked(&self) -> PyResult<MutexGuard<'_, State>> {
        self.state
            .lock()
            .map_err(|_| PyRuntimeError::new_err("event exporter state lock is poisoned"))
    }

    /// Wake every waiting thread; the caller holds the condition.
    pub(crate) fn notify_all(&self, py: Python<'_>) -> PyResult<()> {
        self.condition.bind(py).call_method0("notify_all")?;
        Ok(())
    }

    /// Wait on the held condition for at most `timeout` seconds.
    pub(crate) fn wait(&self, py: Python<'_>, timeout: f64) -> PyResult<()> {
        self.condition
            .bind(py)
            .call_method1("wait", (timeout.max(NO_SECONDS),))?;
        Ok(())
    }

    /// Whether the shutdown drain deadline has passed.
    pub(crate) fn past_deadline(state: &State) -> bool {
        state
            .deadline
            .is_some_and(|deadline| Instant::now() >= deadline)
    }

    fn snapshot(state: &State, final_snapshot: bool) -> ExportSnapshot {
        let exporters: Vec<(String, ExporterCounts)> = state
            .exporters
            .iter()
            .zip(&state.counts)
            .map(|(exporter, counts)| (exporter.name.clone(), *counts))
            .collect();
        export_snapshot(SnapshotInputs {
            exporters: &exporters,
            queue_depth: state.queue.items.len(),
            queue_capacity: state.queue.capacity,
            drained: final_snapshot
                && !state.dispatcher_running
                && state.live_invocations.is_empty()
                && !state.abandoned_calls,
        })
    }

    /// A point-in-time snapshot taken under the condition.
    pub(crate) fn current_snapshot(&self, py: Python<'_>) -> PyResult<ExportSnapshot> {
        let _held = self.hold(py)?;
        Ok(Self::snapshot(&*self.locked()?, false))
    }

    /// The diagnostic queue capacity when failures are reported, else `None`.
    pub(crate) fn diagnostics(&self) -> Option<usize> {
        (self.notifications && self.failure_callback.is_some())
            .then_some(self.notification_capacity)
    }

    /// Forget one finished invocation and wake a caller waiting for delivery to become idle.
    fn invocation_finished(&self, py: Python<'_>, invocation: u64) -> PyResult<()> {
        let retired: Option<Py<PyAny>> = {
            let _held = self.hold(py)?;
            let mut state = self.locked()?;
            state.live_invocations.remove(&invocation);
            let retired: Option<Py<PyAny>> = if Self::idle(&state) && state.shutdown_complete {
                state.output_lane.take()
            } else {
                None
            };
            drop(state);
            self.notify_all(py)?;
            retired
        };
        retire_output_lane(py, retired)
    }

    /// Mark the dispatcher thread finished.
    pub(crate) fn dispatcher_finished(&self, py: Python<'_>) -> PyResult<()> {
        let _held = self.hold(py)?;
        self.locked()?.dispatcher_running = false;
        self.notify_all(py)
    }

    /// Whether the dispatcher thread and every sink call have finished.
    fn idle(state: &State) -> bool {
        state.live_invocations.is_empty() && !state.dispatcher_running
    }

    /// Wait until delivery and every sink call are idle, the idle wait ends or interpreter exit begins.
    fn await_idle(&self, py: Python<'_>) -> PyResult<()> {
        let deadline: Instant =
            Instant::now() + self.invocation_timeout.min(seconds(IDLE_WAIT_SECONDS));
        let threads: Vec<Py<PyAny>> = {
            let _held = self.hold(py)?;
            loop {
                if interpreter_exiting() {
                    return Ok(());
                }
                let state = self.locked()?;
                if Self::idle(&state) {
                    break state
                        .retired
                        .iter()
                        .map(|lane| lane.thread().clone_ref(py))
                        .collect();
                }
                drop(state);
                let remaining: f64 = remaining_seconds(deadline);
                if remaining <= NO_SECONDS {
                    return Ok(());
                }
                self.wait(py, remaining.min(POLL_SECONDS))?;
            }
        };
        for thread in threads {
            join(thread.bind(py), remaining_seconds(deadline))?;
        }
        Ok(())
    }

    /// Register a running invocation, returning its identity.
    fn begin_invocation(&self, py: Python<'_>) -> PyResult<u64> {
        let _held = self.hold(py)?;
        let mut state = self.locked()?;
        state.next_invocation += 1;
        let invocation: u64 = state.next_invocation;
        state.live_invocations.insert(invocation);
        Ok(invocation)
    }

    /// Stop acceptance, delivery, health and notifications so every long-lived thread returns.
    fn stop_for_exit(&self, py: Python<'_>) -> PyResult<()> {
        let _held = self.hold(py)?;
        let mut state = self.locked()?;
        state.accepting = false;
        state.stopping = true;
        state.force_stop = true;
        state.health_stopping = true;
        state.notification_stopping = true;
        drop(state);
        self.notify_all(py)
    }

    /// The queue feeding this dispatcher's single command-output sink thread, started on first use.
    fn output_lane<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let _held = self.hold(py)?;
        let existing: Option<Py<PyAny>> = self
            .locked()?
            .output_lane
            .as_ref()
            .map(|calls| calls.clone_ref(py));
        if let Some(calls) = existing {
            return Ok(calls.into_bound(py));
        }
        let calls: Bound<'py, PyAny> = imported(py, "queue", "SimpleQueue")?.call0()?;
        let arguments = PyDict::new(py);
        arguments.set_item("calls", &calls)?;
        let thread = call_with(
            &imported(py, "threading", "Thread")?,
            &[
                (
                    "target",
                    host::entry(py, UserCodeHost::RunCommandOutputSinks)?,
                ),
                ("kwargs", arguments.into_any()),
                (
                    "name",
                    PyString::new(py, OUTPUT_SINK_THREAD_NAME).into_any(),
                ),
                ("daemon", PyBool::new(py, true).to_owned().into_any()),
            ],
        )?;
        thread.setattr(OUTPUT_SUPPRESSED_ATTRIBUTE, true)?;
        thread.call_method0("start")?;
        self.locked()?.output_lane = Some(calls.clone().unbind());
        Ok(calls)
    }

    /// Call one command-output sink on the sink thread and wait until it returns or exit begins.
    fn call_output_sink(
        &self,
        py: Python<'_>,
        sink: &OutputSink,
        record: &Bound<'_, PyAny>,
    ) -> PyResult<Delivery> {
        let Ok(calls) = self.output_lane(py) else {
            return Ok(Delivery::Delivered(true));
        };
        let outcome: Bound<'_, PyAny> = imported(py, "queue", "SimpleQueue")?.call0()?;
        calls.call_method1(
            "put",
            ((
                sink.function.bind(py),
                sink.provider_arguments.bind(py),
                record,
                &outcome,
            ),),
        )?;
        let empty: Bound<'_, PyAny> = imported(py, "queue", "Empty")?;
        let get: Bound<'_, PyAny> = native_field(&outcome, "get")?;
        loop {
            match call_with(
                &get,
                &[("timeout", PyFloat::new(py, POLL_SECONDS).into_any())],
            ) {
                Ok(failed) => return Ok(Delivery::Delivered(failed.is_truthy()?)),
                Err(error) if error.is_instance(py, &empty) => {}
                Err(error) => return Err(error),
            }
            if interpreter_exiting() {
                return Ok(Delivery::Abandoned);
            }
        }
    }

    /// The final summary once another caller's shutdown completes.
    fn await_shutdown(&self, py: Python<'_>) -> PyResult<Py<EventExportSummary>> {
        let _held = self.hold(py)?;
        loop {
            let state = self.locked()?;
            if state.shutdown_complete {
                return state
                    .final_summary
                    .as_ref()
                    .map(|summary| summary.clone_ref(py))
                    .ok_or_else(|| {
                        state_error(py, "event exporter shutdown completed without summary")
                    });
            }
            drop(state);
            self.wait(py, POLL_SECONDS)?;
        }
    }
}

/// A started daemon thread running `worker`, tagged so the exit hook can stop it.
fn start_worker<'py>(
    py: Python<'py>,
    shared: &Arc<Shared>,
    (role, name): (WorkerRole, &str),
) -> PyResult<Bound<'py, PyAny>> {
    let worker: Bound<'py, PyAny> = Py::new(py, DispatcherWorker::new(Arc::clone(shared), role))?
        .into_bound(py)
        .into_any();
    let thread = call_with(
        &imported(py, "threading", "Thread")?,
        &[
            ("target", worker.clone()),
            ("name", PyString::new(py, name).into_any()),
            ("daemon", PyBool::new(py, true).to_owned().into_any()),
        ],
    )?;
    thread.setattr(WORKER_ATTRIBUTE, worker)?;
    Ok(thread)
}

#[pymethods]
impl EventExporterDispatcher {
    #[new]
    #[pyo3(signature = (**options))]
    pub(crate) fn new(py: Python<'_>, options: Option<Bound<'_, PyDict>>) -> PyResult<Self> {
        let options: KeywordArguments<'_> = KeywordArguments::read(
            py,
            "EventExporterDispatcher",
            options,
            (
                &[],
                &[
                    "exporters",
                    "queue_capacity",
                    "shutdown_timeout_seconds",
                    "invocation_timeout_seconds",
                    "notification_queue_capacity",
                    "health_interval_seconds",
                    "failure_callback",
                    "summary_callback",
                ],
            ),
        )?;
        let limits: Limits = limits(&options)?;
        let exporters: Bound<'_, PyAny> = options.value("exporters")?;
        let (bound_exporters, filters) = if exporters.is_none() {
            (Vec::new(), Vec::new())
        } else {
            read_exporters(&exporters)?
        };
        let callback = |name: &str| -> PyResult<Option<Py<PyAny>>> {
            let value: Bound<'_, PyAny> = options.value(name)?;
            Ok((!value.is_none()).then(|| value.unbind()))
        };
        let failure_callback: Option<Py<PyAny>> = callback("failure_callback")?;
        let summary_callback: Option<Py<PyAny>> = callback("summary_callback")?;
        let notifications: bool = failure_callback.is_some() || summary_callback.is_some();
        let has_summary: bool = summary_callback.is_some();
        let shared = Arc::new(Shared {
            condition: imported(py, "threading", "Condition")?.call0()?.unbind(),
            results: imported(py, "queue", "SimpleQueue")?.call0()?.unbind(),
            claims: PyDict::new(py).unbind(),
            state: Mutex::new(State {
                bound: !exporters.is_none(),
                counts: vec![ExporterCounts::default(); bound_exporters.len()],
                exporters: bound_exporters,
                filters,
                output_sinks: Arc::new(Vec::new()),
                output_lane: None,
                queue: EventQueue::new(limits.queue_capacity),
                sequence: 0,
                accepting: true,
                deadline: None,
                reserve_at: None,
                reserved: false,
                lanes: Vec::new(),
                tokens: HashMap::new(),
                next_token: 0,
                retired: Vec::new(),
                abandoned_calls: false,
                stopping: false,
                force_stop: false,
                blocked: HashSet::new(),
                stalled: HashMap::new(),
                held: Vec::new(),
                newly_blocked: Vec::new(),
                last_charged: None,
                live_invocations: HashSet::new(),
                next_invocation: 0,
                dispatcher_running: true,
                shutdown_started: false,
                shutdown_complete: false,
                final_summary: None,
                failures: VecDeque::new(),
                notification_stopping: false,
                health_stopping: false,
                pending_periodic: None,
                periodic_inflight: None,
                last_published_periodic: [
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    u64::try_from(limits.queue_capacity).unwrap_or(u64::MAX),
                ],
                final_notification: None,
                final_delivered: false,
            }),
            invocation_timeout: limits.invocation_timeout,
            shutdown_timeout: limits.shutdown_timeout,
            health_interval: limits.health_interval,
            notification_capacity: limits.notification_capacity,
            failure_callback,
            summary_callback,
            notifications,
        });
        let notification_thread: Option<Bound<'_, PyAny>> = notifications
            .then(|| start_worker(py, &shared, (WorkerRole::Notify, NOTIFIER_THREAD_NAME)))
            .transpose()?;
        let health_thread: Option<Bound<'_, PyAny>> = has_summary
            .then(|| start_worker(py, &shared, (WorkerRole::Health, HEALTH_THREAD_NAME)))
            .transpose()?;
        for thread in notification_thread.iter().chain(&health_thread) {
            thread.call_method0("start")?;
        }
        let thread = start_worker(py, &shared, (WorkerRole::Dispatch, DISPATCHER_THREAD_NAME))?;
        thread.call_method0("start")?;
        Ok(Self {
            shared,
            thread: thread.unbind(),
            notification_thread: notification_thread.map(Bound::unbind),
            health_thread: health_thread.map(Bound::unbind),
        })
    }

    #[getter]
    pub(crate) fn thread(&self, py: Python<'_>) -> Py<PyAny> {
        self.thread.clone_ref(py)
    }

    #[getter]
    pub(crate) fn notification_thread(&self, py: Python<'_>) -> Option<Py<PyAny>> {
        self.notification_thread
            .as_ref()
            .map(|thread| thread.clone_ref(py))
    }

    /// Filter and enqueue eligible exporter attempts without waiting.
    fn enqueue(&self, py: Python<'_>, event: &Bound<'_, PyAny>) -> PyResult<()> {
        let event_type: String = native_field(event, "event_type")?.extract()?;
        let policy: ExportPolicy = export_policy(py, &event_type)?;
        let shared = &self.shared;
        let _held = shared.hold(py)?;
        let mut state = shared.locked()?;
        if !state.bound {
            drop(state);
            return Err(state_error(
                py,
                "event exporters must be bound before enqueue",
            ));
        }
        let counts: Vec<ExporterCounts> = std::mem::take(&mut state.counts);
        let (counts, eligible) = route_lifecycle_event(&policy, &state.filters, counts);
        state.counts = counts;
        if eligible.is_empty() {
            return Ok(());
        }
        if !state.accepting {
            state.counts = drop_lifecycle_attempts(std::mem::take(&mut state.counts), &eligible);
            return Ok(());
        }
        let sequence: u64 = state.sequence;
        state.sequence += 1;
        let capacity: usize = state.queue.capacity;
        let mut outcome = queue_lifecycle_event(
            std::mem::replace(&mut state.queue, EventQueue::new(capacity)),
            QueuedEvent {
                sequence,
                item: Queued {
                    event: event.clone().unbind(),
                    kind: policy.kind,
                    severity: policy.severity,
                },
                priority: policy.priority,
                invocation_terminal: INVOCATION_TERMINAL_EVENT_TYPES.contains(&event_type.as_str()),
                outcome: OUTCOME_EVENT_TYPES.contains(&event_type.as_str()),
                eligible_exporters: eligible,
            },
        );
        state.queue = std::mem::replace(&mut outcome.queue, EventQueue::new(capacity));
        for lost in outcome.displaced.iter().chain(&outcome.rejected) {
            state.counts = drop_lifecycle_attempts(
                std::mem::take(&mut state.counts),
                &lost.eligible_exporters,
            );
        }
        drop(state);
        shared.notify_all(py)?;
        drop(outcome);
        Ok(())
    }

    /// Bind validated exporters once before lifecycle publication starts.
    fn bind(&self, py: Python<'_>, exporters: &Bound<'_, PyAny>) -> PyResult<()> {
        let (exporters, filters) = read_exporters(exporters)?;
        let shared = &self.shared;
        let _held = shared.hold(py)?;
        let mut state = shared.locked()?;
        if state.bound {
            drop(state);
            return Err(state_error(py, "event exporters are already bound"));
        }
        state.counts = vec![ExporterCounts::default(); exporters.len()];
        state.exporters = exporters;
        state.filters = filters;
        state.bound = true;
        drop(state);
        shared.notify_all(py)
    }

    /// Bind explicit command-output sinks before capture starts.
    fn bind_command_output_sinks(&self, py: Python<'_>, sinks: &Bound<'_, PyAny>) -> PyResult<()> {
        let sinks: Arc<Vec<OutputSink>> = Arc::new(read_output_sinks(sinks)?);
        let previous = {
            let _held = self.shared.hold(py)?;
            std::mem::replace(&mut self.shared.locked()?.output_sinks, sinks)
        };
        drop(previous);
        Ok(())
    }

    /// Deliver output records through the already-bound destination providers.
    fn export_output(&self, py: Python<'_>, records: &Bound<'_, PyAny>) -> PyResult<()> {
        let shared = &self.shared;
        let invocation: u64 = shared.begin_invocation(py)?;
        let sinks: Arc<Vec<OutputSink>> = {
            let _held = shared.hold(py)?;
            Arc::clone(&shared.locked()?.output_sinks)
        };
        let mut failed: bool = false;
        let delivered = (|| -> PyResult<Delivery> {
            for record in records.try_iter()? {
                let record: Bound<'_, PyAny> = record?;
                let is_output: bool =
                    native_field(&record, "record_type")?.eq(COMMAND_OUTPUT_RECORD_TYPE)?;
                let stream: Option<String> = if is_output {
                    Some(native_field(&record, "stream")?.extract()?)
                } else {
                    None
                };
                for sink in sinks.iter() {
                    if stream
                        .as_ref()
                        .is_some_and(|stream| !sink.streams.contains(stream))
                    {
                        continue;
                    }
                    match shared.call_output_sink(py, sink, &record)? {
                        Delivery::Delivered(sink_failed) => failed |= sink_failed,
                        Delivery::Abandoned => return Ok(Delivery::Abandoned),
                    }
                }
            }
            Ok(Delivery::Delivered(failed))
        })();
        if let Ok(Delivery::Abandoned) = delivered {
            return Err(state_error(
                py,
                "output export stopped because the interpreter is exiting",
            ));
        }
        shared.invocation_finished(py, invocation)?;
        delivered?;
        if failed {
            return Err(state_error(py, "one or more output export attempts failed"));
        }
        Ok(())
    }

    /// Stop acceptance and drain until the fixed shutdown deadline.
    fn shutdown(&self, py: Python<'_>) -> PyResult<Py<EventExportSummary>> {
        let shared = &self.shared;
        let (owns, deadline) = {
            let _held = shared.hold(py)?;
            let mut state = shared.locked()?;
            let owns: bool = !state.shutdown_started;
            if owns {
                state.shutdown_started = true;
                state.accepting = false;
                let started: Instant = Instant::now();
                state.deadline = Some(started + shared.shutdown_timeout);
                state.reserve_at = Some(
                    started
                        + shared
                            .shutdown_timeout
                            .mul_f64(1.0 - OUTCOME_RESERVE_FRACTION),
                );
                state.stopping = true;
                state.health_stopping = true;
            }
            let deadline: Instant = state.deadline.unwrap_or_else(Instant::now);
            drop(state);
            shared.notify_all(py)?;
            (owns, deadline)
        };
        if !owns {
            return shared.await_shutdown(py);
        }
        let thread: &Bound<'_, PyAny> = self.thread.bind(py);
        call_with(
            &native_field(thread, "join")?,
            &[(
                "timeout",
                PyFloat::new(py, remaining_seconds(deadline) + JOIN_GRACE_SECONDS).into_any(),
            )],
        )?;
        if thread.call_method0("is_alive")?.is_truthy()? {
            {
                let _held = shared.hold(py)?;
                shared.locked()?.force_stop = true;
                shared.notify_all(py)?;
            }
            thread.call_method0("join")?;
        }
        let snapshot: ExportSnapshot = {
            let _held = shared.hold(py)?;
            Shared::snapshot(&*shared.locked()?, true)
        };
        let summary: Py<EventExportSummary> = EventExportSummary::object(py, snapshot)?;
        {
            let _held = shared.hold(py)?;
            let mut state = shared.locked()?;
            state.final_summary = Some(summary.clone_ref(py));
            state.shutdown_complete = true;
            if shared.summary_callback.is_some()
                && state.final_notification.is_none()
                && !state.final_delivered
            {
                state.final_notification = Some(summary.clone_ref(py));
            }
            state.notification_stopping = true;
            let retired: Option<Py<PyAny>> = if state.live_invocations.is_empty() {
                state.output_lane.take()
            } else {
                None
            };
            drop(state);
            shared.notify_all(py)?;
            retire_output_lane(py, retired)?;
        }
        if let Some(health) = &self.health_thread {
            join(health.bind(py), EXIT_JOIN_SECONDS)?;
        }
        Ok(summary)
    }

    /// Run `finalizer` on the calling thread, never a daemon delivery thread, once delivery is idle or the idle wait ends.
    fn finalize_when_idle(&self, py: Python<'_>, finalizer: Py<PyAny>) -> PyResult<()> {
        self.shared.await_idle(py)?;
        drop(finalizer.bind(py).call0());
        Ok(())
    }

    /// Return a thread-safe immutable point-in-time snapshot.
    fn summary(&self, py: Python<'_>) -> PyResult<Py<EventExportSummary>> {
        let snapshot: ExportSnapshot = self.shared.current_snapshot(py)?;
        EventExportSummary::object(py, snapshot)
    }
}
