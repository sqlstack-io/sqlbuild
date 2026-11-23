//! Public records of event exporting.

use crate::event_exporting::_helpers::dispatcher::Delivery;
use crate::event_exporting::_helpers::dispatcher::Limits;
use crate::event_exporting::_helpers::dispatcher::OutputSink;
use crate::event_exporting::_helpers::dispatcher::Queued;
use crate::event_exporting::_helpers::dispatcher::Shared;
use crate::event_exporting::_helpers::dispatcher::State;
use crate::event_exporting::_helpers::dispatcher::join;
use crate::event_exporting::_helpers::dispatcher::limits;
use crate::event_exporting::_helpers::dispatcher::read_exporters;
use crate::event_exporting::_helpers::dispatcher::read_output_sinks;
use crate::event_exporting::_helpers::dispatcher::remaining_seconds;
use crate::event_exporting::_helpers::dispatcher::retire_output_lane;
use crate::event_exporting::_helpers::dispatcher::start_worker;
use crate::event_exporting::_helpers::workers::Notification;
use crate::event_exporting::_helpers::workers::Round;
use crate::event_exporting::_helpers::workers::WorkerRole;
use crate::event_exporting::_helpers::workers::warn_blocked;
use pyo3::prelude::Bound;
use pyo3::prelude::Py;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyAnyMethods;
use pyo3::prelude::PyErr;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::prelude::pyclass;
use pyo3::prelude::pymethods;
use pyo3::types::PyDict;
use pyo3::types::PyFloat;
use sqlbuild_core::command_output::constants::COMMAND_OUTPUT_RECORD_TYPE;
use sqlbuild_core::event_export::constants::INVOCATION_START_EVENT_TYPE;
use sqlbuild_core::event_export::constants::INVOCATION_TERMINAL_EVENT_TYPES;
use sqlbuild_core::event_export::constants::OUTCOME_EVENT_TYPES;
use sqlbuild_core::event_export::constants::OUTCOME_RESERVE_FRACTION;
use sqlbuild_core::event_export::main::drop_lifecycle_attempts::drop_lifecycle_attempts;
use sqlbuild_core::event_export::main::queue_lifecycle_event::queue_lifecycle_event;
use sqlbuild_core::event_export::main::route_lifecycle_event::route_lifecycle_event;
use sqlbuild_core::event_export::models::EventQueue;
use sqlbuild_core::event_export::models::ExportPolicy;
use sqlbuild_core::event_export::models::ExportSnapshot;
use sqlbuild_core::event_export::models::ExporterCounts;
use sqlbuild_core::event_export::models::QueuedEvent;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::event_exporting::constants::DISPATCHER_THREAD_NAME;
use sqlbuild_python_base::event_exporting::constants::EXIT_JOIN_SECONDS;
use sqlbuild_python_base::event_exporting::constants::HEALTH_THREAD_NAME;
use sqlbuild_python_base::event_exporting::constants::IDLE_POLL_SECONDS;
use sqlbuild_python_base::event_exporting::constants::JOIN_GRACE_SECONDS;
use sqlbuild_python_base::event_exporting::constants::NO_SECONDS;
use sqlbuild_python_base::event_exporting::constants::NOTIFIER_THREAD_NAME;
use sqlbuild_python_base::event_exporting::constants::POLL_SECONDS;
use sqlbuild_python_base::event_exporting::main::error_type_of::error_type_of;
use sqlbuild_python_base::event_exporting::main::export_policy::export_policy;
use sqlbuild_python_base::event_exporting::main::state_error::state_error;
use sqlbuild_python_base::event_exporting::models::EventExportSummary;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Instant;

/// Deliver filtered lifecycle events off execution threads with bounded memory.
#[pyclass(module = "sqlbuild._native", frozen)]
#[derive(Debug)]
pub struct EventExporterDispatcher {
    pub(crate) shared: Arc<Shared>,
    /// Event kinds this command never exports, such as compile's phase operations.
    pub(crate) withheld_kinds: Vec<String>,
    pub(crate) thread: Py<PyAny>,
    pub(crate) notification_thread: Option<Py<PyAny>>,
    pub(crate) health_thread: Option<Py<PyAny>>,
}

#[pymethods]
impl EventExporterDispatcher {
    #[new]
    #[pyo3(signature = (**options))]
    pub fn new(py: Python<'_>, options: Option<Bound<'_, PyDict>>) -> PyResult<Self> {
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
                    "withheld_kinds",
                ],
            ),
        )?;
        let withheld: Bound<'_, PyAny> = options.value("withheld_kinds")?;
        let withheld_kinds: Vec<String> = if withheld.is_none() {
            Vec::new()
        } else {
            withheld.extract()?
        };
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
            withheld_kinds,
            thread: thread.unbind(),
            notification_thread: notification_thread.map(Bound::unbind),
            health_thread: health_thread.map(Bound::unbind),
        })
    }

    #[getter]
    pub fn thread(&self, py: Python<'_>) -> Py<PyAny> {
        self.thread.clone_ref(py)
    }

    #[getter]
    pub fn notification_thread(&self, py: Python<'_>) -> Option<Py<PyAny>> {
        self.notification_thread
            .as_ref()
            .map(|thread| thread.clone_ref(py))
    }

    /// Filter and enqueue eligible exporter attempts without waiting.
    pub(crate) fn enqueue(&self, py: Python<'_>, event: &Bound<'_, PyAny>) -> PyResult<()> {
        let event_type: String = native_field(event, "event_type")?.extract()?;
        let policy: ExportPolicy = export_policy(py, &event_type)?;
        if self.withheld_kinds.contains(&policy.kind) {
            return Ok(());
        }
        let invocation_id: Option<String> = match native_field(event, "invocation_id") {
            Ok(value) => value.extract()?,
            Err(_) => None,
        };
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
                invocation_id,
                invocation_start: event_type == INVOCATION_START_EVENT_TYPE,
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
    pub(crate) fn bind(&self, py: Python<'_>, exporters: &Bound<'_, PyAny>) -> PyResult<()> {
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
    pub(crate) fn bind_command_output_sinks(
        &self,
        py: Python<'_>,
        sinks: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let sinks: Arc<Vec<OutputSink>> = Arc::new(read_output_sinks(sinks)?);
        let previous = {
            let _held = self.shared.hold(py)?;
            std::mem::replace(&mut self.shared.locked()?.output_sinks, sinks)
        };
        drop(previous);
        Ok(())
    }

    /// Deliver output records through the already-bound destination providers.
    pub(crate) fn export_output(&self, py: Python<'_>, records: &Bound<'_, PyAny>) -> PyResult<()> {
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
    pub(crate) fn shutdown(&self, py: Python<'_>) -> PyResult<Py<EventExportSummary>> {
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

    /// Run `finalizer` on the calling thread, never a daemon delivery thread, once delivery is idle or the idle wait ends; its error is raised.
    pub(crate) fn finalize_when_idle(&self, py: Python<'_>, finalizer: Py<PyAny>) -> PyResult<()> {
        self.shared.await_idle(py)?;
        finalizer.bind(py).call0()?;
        Ok(())
    }

    /// Return a thread-safe immutable point-in-time snapshot.
    pub(crate) fn summary(&self, py: Python<'_>) -> PyResult<Py<EventExportSummary>> {
        let snapshot: ExportSnapshot = self.shared.current_snapshot(py)?;
        EventExportSummary::object(py, snapshot)
    }
}

/// A long-lived dispatcher thread's target.
#[pyclass(module = "sqlbuild._native", frozen)]
pub struct DispatcherWorker {
    pub(crate) shared: Arc<Shared>,
    pub(crate) role: WorkerRole,
}

impl std::fmt::Debug for DispatcherWorker {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DispatcherWorker")
            .finish_non_exhaustive()
    }
}

impl DispatcherWorker {
    pub(crate) fn new(shared: Arc<Shared>, role: WorkerRole) -> Self {
        Self { shared, role }
    }

    /// The state this worker's dispatcher shares.
    pub(crate) fn shared(&self) -> &Shared {
        &self.shared
    }

    /// Wait until bound, returning `false` when stopped first.
    pub(crate) fn await_bound(&self, py: Python<'_>) -> PyResult<bool> {
        let shared = &self.shared;
        let _held = shared.hold(py)?;
        loop {
            let state = shared.locked()?;
            if state.bound {
                return Ok(true);
            }
            if state.stopping || state.force_stop {
                return Ok(false);
            }
            drop(state);
            shared.wait(py, POLL_SECONDS)?;
        }
    }

    pub(crate) fn run_dispatch(&self, py: Python<'_>) -> PyResult<()> {
        let result = (|| -> PyResult<()> {
            if !self.await_bound(py)? {
                return Ok(());
            }
            while self.dispatch_round(py)? {}
            Ok(())
        })();
        let retired: PyResult<()> = (|| {
            let _held = self.shared.hold(py)?;
            self.shared.locked()?.retire_lanes(py)
        })();
        self.shared.dispatcher_finished(py)?;
        retired?;
        result
    }

    /// Record reported outcomes, expire, trim and feed lanes, then wait; `false` once delivery stops.
    pub(crate) fn dispatch_round(&self, py: Python<'_>) -> PyResult<bool> {
        let shared = &self.shared;
        let round = Round {
            shared,
            diagnostics: shared.diagnostics(),
        };
        let _held = shared.hold(py)?;
        let reports: Vec<(u64, Option<String>)> = self.reported(py)?;
        let mut state = shared.locked()?;
        let mut notify: bool = state.record_reports(py, reports, round)?;
        let stopped: bool = if Shared::past_deadline(&state) || state.force_stop {
            let drained: Vec<QueuedEvent<Queued>> = std::mem::take(&mut state.queue.items);
            for item in &drained {
                state.counts = drop_lifecycle_attempts(
                    std::mem::take(&mut state.counts),
                    &item.eligible_exporters,
                );
            }
            for index in 0..state.lanes.len() {
                notify |= state.abandon_lane(py, (index, false), round)?;
            }
            for (index, _) in std::mem::take(&mut state.held) {
                state.counts = drop_lifecycle_attempts(std::mem::take(&mut state.counts), &[index]);
            }
            drop(drained);
            true
        } else {
            notify |= state.expire_lanes(py, shared.invocation_timeout, round)?;
            if !state.reserved && state.reserve_at.is_some_and(|at| Instant::now() >= at) {
                state.reserved = true;
                state.reserve_for_outcomes(py, &shared.claims)?;
            }
            notify |= state.feed(py, round)?;
            state.stopping && !state.busy() && state.queue.items.is_empty() && state.held.is_empty()
        };
        let idle: bool = !stopped && Self::idle_round(&state);
        let blocked: Vec<String> = std::mem::take(&mut state.newly_blocked);
        drop(state);
        for name in blocked {
            warn_blocked(py, &name, shared.invocation_timeout);
        }
        if notify {
            shared.notify_all(py)?;
        }
        if !stopped {
            shared.wait(
                py,
                if idle {
                    IDLE_POLL_SECONDS
                } else {
                    POLL_SECONDS
                },
            )?;
        }
        Ok(!stopped)
    }

    /// Whether a round has nothing to watch: no queued, held or pending work, and no shutdown.
    fn idle_round(state: &State) -> bool {
        !state.busy()
            && state.queue.items.is_empty()
            && state.held.is_empty()
            && !state.stopping
            && !state.force_stop
            && state.deadline.is_none()
    }

    /// Every `(token, sanitized error type)` delivery threads reported since the last round.
    pub(crate) fn reported(&self, py: Python<'_>) -> PyResult<Vec<(u64, Option<String>)>> {
        let results: &Bound<'_, PyAny> = self.shared.results.bind(py);
        let mut reports: Vec<(u64, Option<String>)> = Vec::new();
        while !results.call_method0("empty")?.is_truthy()? {
            let (token, error): (u64, Bound<'_, PyAny>) =
                results.call_method0("get_nowait")?.extract()?;
            let error_type: Option<String> =
                (!error.is_none()).then(|| error_type_of(py, &PyErr::from_value(error)));
            reports.push((token, error_type));
        }
        Ok(reports)
    }

    pub(crate) fn next_notification(&self, py: Python<'_>) -> PyResult<Notification> {
        let shared = &self.shared;
        let _held = shared.hold(py)?;
        loop {
            let mut state = shared.locked()?;
            if let Some(snapshot) = state.pending_periodic.take() {
                state.periodic_inflight = Some(snapshot.health_signature());
                return Ok(Notification::Periodic(snapshot));
            }
            state.periodic_inflight = None;
            if !state.final_delivered
                && let Some(summary) = &state.final_notification
            {
                return Ok(Notification::Final(summary.clone_ref(py)));
            }
            if let Some(failure) = state.failures.pop_front() {
                return Ok(Notification::Failure(failure));
            }
            if state.notification_stopping {
                return Ok(Notification::Stop);
            }
            drop(state);
            shared.wait(py, IDLE_POLL_SECONDS)?;
        }
    }

    pub(crate) fn run_notifications(&self, py: Python<'_>) -> PyResult<()> {
        let shared = &self.shared;
        loop {
            match self.next_notification(py)? {
                Notification::Periodic(snapshot) => {
                    let signature: [u64; 7] = snapshot.health_signature();
                    if let Some(callback) = &shared.summary_callback {
                        let summary = EventExportSummary::object(py, snapshot)?;
                        drop(callback.bind(py).call1((summary,)));
                    }
                    let _held = shared.hold(py)?;
                    let mut state = shared.locked()?;
                    state.last_published_periodic = signature;
                    state.periodic_inflight = None;
                }
                Notification::Final(summary) => {
                    if let Some(callback) = &shared.summary_callback {
                        drop(callback.bind(py).call1((summary,)));
                    }
                    let released = {
                        let _held = shared.hold(py)?;
                        let mut state = shared.locked()?;
                        state.final_delivered = true;
                        state.final_notification.take()
                    };
                    drop(released);
                    return Ok(());
                }
                Notification::Failure(failure) => {
                    if let Some(callback) = &shared.failure_callback {
                        let failure = Py::new(py, failure)?;
                        drop(callback.bind(py).call1((failure,)));
                    }
                }
                Notification::Stop => return Ok(()),
            }
        }
    }

    /// Wait one health interval, returning `false` once health publication stops.
    pub(crate) fn await_interval(&self, py: Python<'_>) -> PyResult<bool> {
        let shared = &self.shared;
        let deadline: Instant = Instant::now() + shared.health_interval;
        let _held = shared.hold(py)?;
        loop {
            if shared.locked()?.health_stopping {
                return Ok(false);
            }
            let remaining: f64 = deadline
                .saturating_duration_since(Instant::now())
                .as_secs_f64();
            if remaining <= NO_SECONDS {
                return Ok(true);
            }
            shared.wait(py, remaining)?;
        }
    }

    pub(crate) fn run_health(&self, py: Python<'_>) -> PyResult<()> {
        let shared = &self.shared;
        while self.await_interval(py)? {
            let snapshot: ExportSnapshot = shared.current_snapshot(py)?;
            let signature: [u64; 7] = snapshot.health_signature();
            let _held = shared.hold(py)?;
            let mut state = shared.locked()?;
            if state.health_stopping || state.final_notification.is_some() {
                return Ok(());
            }
            if signature == state.last_published_periodic
                || state.periodic_inflight == Some(signature)
            {
                state.pending_periodic = None;
                continue;
            }
            state.pending_periodic = Some(snapshot);
            drop(state);
            shared.notify_all(py)?;
        }
        Ok(())
    }
}

#[pymethods]
impl DispatcherWorker {
    pub(crate) fn __call__(&self, py: Python<'_>) -> PyResult<()> {
        match self.role {
            WorkerRole::Dispatch => self.run_dispatch(py),
            WorkerRole::Notify => self.run_notifications(py),
            WorkerRole::Health => self.run_health(py),
        }
    }
}
