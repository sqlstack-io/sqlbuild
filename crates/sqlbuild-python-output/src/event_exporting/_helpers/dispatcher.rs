//! `EventExporterDispatcher`: bounded priority delivery of lifecycle events on Python daemon threads.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python, pyfunction};
use pyo3::types::{PyBool, PyDict, PyFloat, PyInt, PyString};
use sqlbuild_core::event_export::constants::{
    DEFAULT_EVENT_EXPORT_HEALTH_INTERVAL_SECONDS, DEFAULT_EVENT_EXPORT_INVOCATION_TIMEOUT_SECONDS,
    DEFAULT_EVENT_EXPORT_NOTIFICATION_QUEUE_CAPACITY, DEFAULT_EVENT_EXPORT_QUEUE_CAPACITY,
    DEFAULT_EVENT_EXPORT_SHUTDOWN_TIMEOUT_SECONDS,
};
use sqlbuild_core::event_export::main::export_snapshot::export_snapshot;
use sqlbuild_core::event_export::models::{
    EventQueue, ExportSeverity, ExportSnapshot, ExporterCounts, ExporterFilter, SnapshotInputs,
};
use sqlbuild_host::entries::models::UserCodeHost;

use crate::event_exporting::_helpers::workers::Lane;
use crate::event_exporting::_helpers::workers::WorkerRole;
use crate::event_exporting::models::DispatcherWorker;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::event_exporting::constants::EXIT_JOIN_SECONDS;
use sqlbuild_python_base::event_exporting::constants::IDLE_WAIT_SECONDS;
use sqlbuild_python_base::event_exporting::constants::NO_SECONDS;
use sqlbuild_python_base::event_exporting::constants::OUTPUT_SINK_THREAD_NAME;
use sqlbuild_python_base::event_exporting::constants::POLL_SECONDS;
use sqlbuild_python_base::event_exporting::constants::WORKER_ATTRIBUTE;
use sqlbuild_python_base::event_exporting::main::input_error::input_error;
use sqlbuild_python_base::event_exporting::main::severity_from::severity_from;
use sqlbuild_python_base::event_exporting::main::state_error::state_error;
use sqlbuild_python_base::event_exporting::models::BoundEventExporter;
use sqlbuild_python_base::event_exporting::models::EventExportSummary;
use sqlbuild_python_base::event_exporting::models::EventExporterFailure;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::native_threads::constants::OUTPUT_SUPPRESSED_ATTRIBUTE;
use sqlbuild_python_base::native_threads::main::interpreter_exiting::interpreter_exiting;
use sqlbuild_python_base::native_threads::main::signal_interpreter_exit::signal_interpreter_exit;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// One bound lifecycle-event sink.
#[derive(Debug)]
pub(crate) struct Exporter {
    pub(crate) name: String,
    pub(crate) function: Py<PyAny>,
    pub(crate) provider_arguments: Py<PyAny>,
}

/// One bound command-output sink and the streams it accepts.
#[derive(Debug)]
pub(crate) struct OutputSink {
    pub(crate) function: Py<PyAny>,
    pub(crate) provider_arguments: Py<PyAny>,
    pub(crate) streams: Vec<String>,
}

/// A queued event with the policy dimensions its failures report.
#[derive(Debug)]
pub(crate) struct Queued {
    pub(crate) event: Py<PyAny>,
    pub(crate) kind: String,
    pub(crate) severity: ExportSeverity,
}

/// Delivery state, read and written only while holding the dispatcher's Python condition.
#[derive(Debug)]
pub(crate) struct State {
    pub(crate) bound: bool,
    pub(crate) exporters: Vec<Arc<Exporter>>,
    pub(crate) filters: Vec<ExporterFilter>,
    pub(crate) counts: Vec<ExporterCounts>,
    pub(crate) output_sinks: Arc<Vec<OutputSink>>,
    /// Calls queued to the single command-output sink thread, once it has started.
    pub(crate) output_lane: Option<Py<PyAny>>,
    pub(crate) queue: EventQueue<Queued>,
    pub(crate) sequence: u64,
    pub(crate) accepting: bool,
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
    pub(crate) dispatcher_running: bool,
    pub(crate) shutdown_started: bool,
    pub(crate) shutdown_complete: bool,
    pub(crate) final_summary: Option<Py<EventExportSummary>>,
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
#[derive(Debug)]
pub(crate) struct Shared {
    pub(crate) condition: Py<PyAny>,
    /// The `SimpleQueue` every delivery thread reports `(token, error)` outcomes to.
    pub(crate) results: Py<PyAny>,
    /// Attempt tokens a delivery thread claimed (`True`) before running, or the dispatcher cancelled (`False`).
    pub(crate) claims: Py<PyDict>,
    pub(crate) state: Mutex<State>,
    pub(crate) invocation_timeout: Duration,
    pub(crate) shutdown_timeout: Duration,
    pub(crate) health_interval: Duration,
    pub(crate) notification_capacity: usize,
    pub(crate) failure_callback: Option<Py<PyAny>>,
    pub(crate) summary_callback: Option<Py<PyAny>>,
    pub(crate) notifications: bool,
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
pub(crate) struct Limits {
    pub(crate) queue_capacity: usize,
    pub(crate) shutdown_timeout: Duration,
    pub(crate) invocation_timeout: Duration,
    pub(crate) notification_capacity: usize,
    pub(crate) health_interval: Duration,
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

pub(crate) fn remaining_seconds(deadline: Instant) -> f64 {
    deadline
        .saturating_duration_since(Instant::now())
        .as_secs_f64()
}

/// End a retired command-output sink thread once it has run every call queued before.
pub(crate) fn retire_output_lane(py: Python<'_>, retired: Option<Py<PyAny>>) -> PyResult<()> {
    if let Some(calls) = retired {
        calls.bind(py).call_method1("put", (py.None(),))?;
    }
    Ok(())
}

/// `thread.join(timeout=seconds)` when the thread is alive.
pub(crate) fn join(thread: &Bound<'_, PyAny>, seconds: f64) -> PyResult<()> {
    if thread.call_method0("is_alive")?.is_truthy()? {
        call_with(
            &native_field(thread, "join")?,
            &[("timeout", PyFloat::new(thread.py(), seconds).into_any())],
        )?;
    }
    Ok(())
}

/// Bound exporters read from `BoundEventExporter` values or objects with the same attributes.
pub(crate) fn read_exporters(
    exporters: &Bound<'_, PyAny>,
) -> PyResult<(Vec<Arc<Exporter>>, Vec<ExporterFilter>)> {
    let mut bound: Vec<Arc<Exporter>> = Vec::new();
    let mut filters: Vec<ExporterFilter> = Vec::new();
    for exporter in exporters.try_iter()? {
        let exporter: Bound<'_, PyAny> = exporter?;
        if let Ok(native) = exporter.cast::<BoundEventExporter>() {
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

pub(crate) fn read_output_sinks(sinks: &Bound<'_, PyAny>) -> PyResult<Vec<OutputSink>> {
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
pub(crate) fn health_interval(value: &Bound<'_, PyAny>) -> PyResult<f64> {
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
pub(crate) fn limits(options: &KeywordArguments<'_>) -> PyResult<Limits> {
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
        if let Ok(worker) = worker.cast::<DispatcherWorker>() {
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

    pub(crate) fn snapshot(state: &State, final_snapshot: bool) -> ExportSnapshot {
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
    pub(crate) fn invocation_finished(&self, py: Python<'_>, invocation: u64) -> PyResult<()> {
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
    pub(crate) fn idle(state: &State) -> bool {
        state.live_invocations.is_empty() && !state.dispatcher_running
    }

    /// Wait until delivery and every sink call are idle, the idle wait ends or interpreter exit begins.
    pub(crate) fn await_idle(&self, py: Python<'_>) -> PyResult<()> {
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
    pub(crate) fn begin_invocation(&self, py: Python<'_>) -> PyResult<u64> {
        let _held = self.hold(py)?;
        let mut state = self.locked()?;
        state.next_invocation += 1;
        let invocation: u64 = state.next_invocation;
        state.live_invocations.insert(invocation);
        Ok(invocation)
    }

    /// Stop acceptance, delivery, health and notifications so every long-lived thread returns.
    pub(crate) fn stop_for_exit(&self, py: Python<'_>) -> PyResult<()> {
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
    pub(crate) fn output_lane<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
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
                    host_entry(py, UserCodeHost::RunCommandOutputSinks)?,
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
    pub(crate) fn call_output_sink(
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
    pub(crate) fn await_shutdown(&self, py: Python<'_>) -> PyResult<Py<EventExportSummary>> {
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
pub(crate) fn start_worker<'py>(
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
