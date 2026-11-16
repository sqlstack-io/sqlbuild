//! The dispatcher's delivery, notification and health thread targets, and each exporter's delivery lane.

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};

use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyErr, PyResult, Python, pyclass, pymethods};
use pyo3::types::{PyBool, PyDict, PyDictMethods, PyString};
use sqlbuild_core::event_export::constants::TIMEOUT_ERROR_TYPE;
use sqlbuild_core::event_export::main::drop_lifecycle_attempts::drop_lifecycle_attempts;
use sqlbuild_core::event_export::main::retain_outcome_events::retain_outcome_events;
use sqlbuild_core::event_export::main::take_lifecycle_event::take_lifecycle_event;
use sqlbuild_core::event_export::models::{EventQueue, ExportSnapshot, QueuedEvent};
use sqlbuild_host::entries::models::UserCodeHost;

use crate::bindings::_helpers::event_exporting::dispatcher::{Exporter, Queued, Shared, State};
use crate::bindings::_helpers::recall::console::write_stderr;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::event_exporting::constants::CHARGE_LIMIT_SECONDS;
use sqlbuild_python_base::event_exporting::constants::INVOCATION_THREAD_PREFIX;
use sqlbuild_python_base::event_exporting::constants::LANE_WINDOW;
use sqlbuild_python_base::event_exporting::constants::NO_SECONDS;
use sqlbuild_python_base::event_exporting::constants::POLL_SECONDS;
use sqlbuild_python_base::event_exporting::main::error_type_of::error_type_of;
use sqlbuild_python_base::event_exporting::models::EventExportSummary;
use sqlbuild_python_base::event_exporting::models::EventExporterFailure;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;

/// Which long-lived dispatcher thread a worker runs.
#[derive(Clone, Copy)]
pub(crate) enum WorkerRole {
    Dispatch,
    Notify,
    Health,
}

/// A long-lived dispatcher thread's target.
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct DispatcherWorker {
    shared: Arc<Shared>,
    role: WorkerRole,
}

/// Outcome events held for blocked exporters, each with its exporter index.
type HeldOutcomes = Vec<(usize, Queued)>;

/// One attempt handed to a lane and not yet reported.
pub(crate) struct Attempt {
    token: u64,
    queued: Queued,
    outcome: bool,
}

/// One exporter's long-lived delivery thread, its call queue and the attempts it still owes.
pub(crate) struct Lane {
    calls: Py<PyAny>,
    thread: Py<PyAny>,
    pending: VecDeque<Attempt>,
    /// The running attempt at the front of `pending` and the time charged to it so far.
    timed: Option<(u64, Duration)>,
}

/// What one dispatch round reads from the dispatcher besides its state.
#[derive(Clone, Copy)]
pub(crate) struct Round<'a> {
    shared: &'a Shared,
    diagnostics: Option<usize>,
}

/// The next thing the notification thread delivers.
enum Notification {
    Periodic(ExportSnapshot),
    Final(Py<EventExportSummary>),
    Failure(EventExporterFailure),
    Stop,
}

impl Lane {
    /// Start `exporter`'s delivery thread, which claims each call in `claims` and reports to `results`.
    fn start(py: Python<'_>, exporter: &Exporter, round: Round<'_>) -> PyResult<Self> {
        let calls: Bound<'_, PyAny> = imported(py, "queue", "SimpleQueue")?.call0()?;
        let arguments = PyDict::new(py);
        arguments.set_item("calls", &calls)?;
        arguments.set_item("results", round.shared.results.bind(py))?;
        arguments.set_item("claims", round.shared.claims.bind(py))?;
        let thread = call_with(
            &imported(py, "threading", "Thread")?,
            &[
                (
                    "target",
                    host_entry(py, UserCodeHost::InvokeLifecycleEventSink)?,
                ),
                ("kwargs", arguments.into_any()),
                (
                    "name",
                    PyString::new(py, &format!("{INVOCATION_THREAD_PREFIX}{}", exporter.name))
                        .into_any(),
                ),
                ("daemon", PyBool::new(py, true).to_owned().into_any()),
            ],
        )?;
        thread.call_method0("start")?;
        Ok(Self {
            calls: calls.unbind(),
            thread: thread.unbind(),
            pending: VecDeque::new(),
            timed: None,
        })
    }

    /// The lane's delivery thread.
    pub(crate) fn thread(&self) -> &Py<PyAny> {
        &self.thread
    }

    /// Queue `None` so the thread ends after the calls queued before it.
    fn retire(&self, py: Python<'_>) -> PyResult<()> {
        self.calls.bind(py).call_method1("put", (py.None(),))?;
        Ok(())
    }
}

impl State {
    /// Account reported attempts against their lanes, returning whether a diagnostic was queued.
    fn record_reports(
        &mut self,
        py: Python<'_>,
        reports: Vec<(u64, Option<String>)>,
        round: Round<'_>,
    ) -> PyResult<bool> {
        let claims: &Bound<'_, PyDict> = round.shared.claims.bind(py);
        let mut notify: bool = false;
        for (token, error_type) in reports {
            if claims.contains(token)? {
                claims.del_item(token)?;
            }
            let Some(index) = self.tokens.remove(&token) else {
                if let Some(index) = self.stalled.remove(&token) {
                    self.abandoned_calls = !self.stalled.is_empty();
                    notify |= self.resume(py, index, round)?;
                }
                continue;
            };
            let Some(lane) = self.lanes.get_mut(index).and_then(Option::as_mut) else {
                continue;
            };
            let Some(position) = lane
                .pending
                .iter()
                .position(|attempt| attempt.token == token)
            else {
                continue;
            };
            let Some(attempt) = lane.pending.remove(position) else {
                continue;
            };
            match error_type {
                Some(error_type) => {
                    notify |= self.note_failure(
                        round.diagnostics,
                        (index, &attempt.queued.kind, attempt.queued.severity),
                        error_type,
                    );
                }
                None => {
                    if let Some(counts) = self.counts.get_mut(index) {
                        counts.delivered += 1;
                    }
                }
            }
        }
        Ok(notify)
    }

    /// Cancel `attempts` the delivery thread has not claimed (checked and marked under the GIL), counting each dropped; returns the claimed.
    fn cancel_unclaimed(
        &mut self,
        py: Python<'_>,
        index: usize,
        attempts: impl IntoIterator<Item = Attempt>,
        claims: &Py<PyDict>,
    ) -> PyResult<Vec<Attempt>> {
        let claims: &Bound<'_, PyDict> = claims.bind(py);
        let mut claimed: Vec<Attempt> = Vec::new();
        for attempt in attempts {
            if claim(claims, attempt.token)? == Some(true) {
                claimed.push(attempt);
                continue;
            }
            cancel(claims, attempt.token)?;
            self.tokens.remove(&attempt.token);
            self.counts = drop_lifecycle_attempts(std::mem::take(&mut self.counts), &[index]);
        }
        Ok(claimed)
    }

    /// Unblock `index` once its stalled call returned, delivering its held outcomes on a fresh lane.
    fn resume(&mut self, py: Python<'_>, index: usize, round: Round<'_>) -> PyResult<bool> {
        if self.stalled.values().any(|stalled| *stalled == index) {
            return Ok(false);
        }
        self.blocked.remove(&index);
        let (owed, kept): (HeldOutcomes, HeldOutcomes) = std::mem::take(&mut self.held)
            .into_iter()
            .partition(|(exporter, _)| *exporter == index);
        self.held = kept;
        let mut notify: bool = false;
        for (_, queued) in owed {
            notify |= self.offer(py, index, (&queued, true), round)?;
        }
        Ok(notify)
    }

    /// Retire `index`'s lane: claimed calls fail as timed out and block it until they return.
    fn abandon_lane(
        &mut self,
        py: Python<'_>,
        (index, timed_out): (usize, bool),
        round: Round<'_>,
    ) -> PyResult<bool> {
        let Some(mut lane) = self.lanes.get_mut(index).and_then(Option::take) else {
            return Ok(false);
        };
        let claims: &Bound<'_, PyDict> = round.shared.claims.bind(py);
        let mut attempts: Vec<Attempt> = Vec::new();
        for attempt in std::mem::take(&mut lane.pending) {
            if timed_out && attempt.outcome && claim(claims, attempt.token)?.is_none() {
                cancel(claims, attempt.token)?;
                self.tokens.remove(&attempt.token);
                self.held.push((index, attempt.queued));
            } else {
                attempts.push(attempt);
            }
        }
        let claimed: Vec<Attempt> =
            self.cancel_unclaimed(py, index, attempts, &round.shared.claims)?;
        let mut notify: bool = false;
        for running in claimed {
            claims.del_item(running.token)?;
            self.tokens.remove(&running.token);
            self.stalled.insert(running.token, index);
            if self.blocked.insert(index)
                && timed_out
                && let Some(exporter) = self.exporters.get(index)
            {
                self.newly_blocked.push(exporter.name.clone());
            }
            self.abandoned_calls = true;
            notify |= self.note_failure(
                round.diagnostics,
                (index, &running.queued.kind, running.queued.severity),
                TIMEOUT_ERROR_TYPE.to_owned(),
            );
        }
        lane.retire(py)?;
        self.retired.push(lane);
        Ok(notify)
    }

    /// Abandon lanes whose started call outlived `timeout`, charging each round at most `CHARGE_LIMIT_SECONDS`.
    fn expire_lanes(
        &mut self,
        py: Python<'_>,
        timeout: Duration,
        round: Round<'_>,
    ) -> PyResult<bool> {
        let now: Instant = Instant::now();
        let charge: Duration = self.last_charged.map_or(Duration::ZERO, |last| {
            now.saturating_duration_since(last)
                .min(Duration::from_secs_f64(CHARGE_LIMIT_SECONDS))
        });
        self.last_charged = Some(now);
        let claims: &Bound<'_, PyDict> = round.shared.claims.bind(py);
        let mut notify: bool = false;
        for index in 0..self.lanes.len() {
            let Some(lane) = self.lanes[index].as_mut() else {
                continue;
            };
            let running: Option<u64> = match lane.pending.front() {
                Some(head) if claim(claims, head.token)? == Some(true) => Some(head.token),
                _ => None,
            };
            lane.timed = running.map(|token| match lane.timed {
                Some((timed, charged)) if timed == token => (token, charged + charge),
                _ => (token, Duration::ZERO),
            });
            if lane.timed.is_some_and(|(_, charged)| charged >= timeout) {
                notify |= self.abandon_lane(py, (index, true), round)?;
            }
        }
        Ok(notify)
    }

    /// Drop every queued and lane-waiting event that is not an outcome, counting each attempt.
    fn reserve_for_outcomes(&mut self, py: Python<'_>, claims: &Py<PyDict>) -> PyResult<()> {
        let capacity: usize = self.queue.capacity;
        let (queue, dropped) = retain_outcome_events(std::mem::replace(
            &mut self.queue,
            EventQueue::new(capacity),
        ));
        self.queue = queue;
        for item in &dropped {
            self.counts =
                drop_lifecycle_attempts(std::mem::take(&mut self.counts), &item.eligible_exporters);
        }
        for index in 0..self.lanes.len() {
            let Some(lane) = self.lanes[index].as_mut() else {
                continue;
            };
            let (kept, skipped): (VecDeque<Attempt>, VecDeque<Attempt>) =
                lane.pending.drain(..).partition(|attempt| attempt.outcome);
            let claimed: Vec<Attempt> = self.cancel_unclaimed(py, index, skipped, claims)?;
            let Some(lane) = self.lanes[index].as_mut() else {
                continue;
            };
            let mut pending: Vec<Attempt> = claimed.into_iter().chain(kept).collect();
            pending.sort_by_key(|attempt| attempt.token);
            lane.pending = pending.into();
        }
        drop(dropped);
        Ok(())
    }

    /// Move queued events onto exporter lanes while every eligible lane has room.
    fn feed(&mut self, py: Python<'_>, round: Round<'_>) -> PyResult<bool> {
        let mut notify: bool = false;
        while !self.queue.items.is_empty() {
            let capacity: usize = self.queue.capacity;
            let (queue, next) = take_lifecycle_event(std::mem::replace(
                &mut self.queue,
                EventQueue::new(capacity),
            ));
            self.queue = queue;
            let Some(item) = next else {
                break;
            };
            let full: bool = item.eligible_exporters.iter().any(|index| {
                !self.blocked.contains(index)
                    && self
                        .lanes
                        .get(*index)
                        .and_then(Option::as_ref)
                        .is_some_and(|lane| lane.pending.len() >= LANE_WINDOW.min(capacity))
            });
            if full {
                self.queue.items.push(item);
                break;
            }
            for index in &item.eligible_exporters {
                if self.blocked.contains(index) {
                    if item.outcome {
                        let queued: Queued = Queued {
                            event: item.item.event.clone_ref(py),
                            kind: item.item.kind.clone(),
                            severity: item.item.severity,
                        };
                        self.held.push((*index, queued));
                    } else {
                        self.counts =
                            drop_lifecycle_attempts(std::mem::take(&mut self.counts), &[*index]);
                    }
                    continue;
                }
                notify |= self.offer(py, *index, (&item.item, item.outcome), round)?;
            }
        }
        Ok(notify)
    }

    /// Queue one attempt on `index`'s lane, starting the lane's thread on first use.
    fn offer(
        &mut self,
        py: Python<'_>,
        index: usize,
        (item, outcome): (&Queued, bool),
        round: Round<'_>,
    ) -> PyResult<bool> {
        let Some(exporter) = self.exporters.get(index).map(Arc::clone) else {
            return Ok(false);
        };
        if self.lanes.len() <= index {
            self.lanes.resize_with(index + 1, || None);
        }
        if self.lanes[index].is_none() {
            match Lane::start(py, &exporter, round) {
                Ok(lane) => self.lanes[index] = Some(lane),
                Err(error) => {
                    return Ok(self.note_failure(
                        round.diagnostics,
                        (index, &item.kind, item.severity),
                        error_type_of(py, &error),
                    ));
                }
            }
        }
        let token: u64 = self.next_token;
        self.next_token += 1;
        let Some(lane) = self.lanes[index].as_mut() else {
            return Ok(false);
        };
        lane.calls.bind(py).call_method1(
            "put",
            ((
                exporter.function.bind(py),
                exporter.provider_arguments.bind(py),
                item.event.bind(py),
                token,
            ),),
        )?;
        lane.pending.push_back(Attempt {
            token,
            queued: Queued {
                event: item.event.clone_ref(py),
                kind: item.kind.clone(),
                severity: item.severity,
            },
            outcome,
        });
        self.tokens.insert(token, index);
        Ok(false)
    }

    /// Whether any lane still owes a reported attempt.
    fn busy(&self) -> bool {
        self.lanes
            .iter()
            .flatten()
            .any(|lane| !lane.pending.is_empty())
    }

    /// End every lane once it has run what was queued before, keeping its thread to join.
    fn retire_lanes(&mut self, py: Python<'_>) -> PyResult<()> {
        let lanes: Vec<Lane> = self.lanes.drain(..).flatten().collect();
        for lane in lanes {
            lane.retire(py)?;
            self.retired.push(lane);
        }
        Ok(())
    }
}

/// Whether a delivery thread claimed `token` (`Some(true)`), it was cancelled, or neither (`None`).
fn claim(claims: &Bound<'_, PyDict>, token: u64) -> PyResult<Option<bool>> {
    claims
        .get_item(token)?
        .map(|claim| claim.is_truthy())
        .transpose()
}

/// Mark `token` cancelled, so its delivery thread skips the call.
fn cancel(claims: &Bound<'_, PyDict>, token: u64) -> PyResult<()> {
    claims.set_item(token, false)
}

/// Warn once per stall that a sink call outlived its timeout; best-effort, ignoring write errors.
fn warn_blocked(py: Python<'_>, name: &str, timeout: Duration) {
    drop(write_stderr(
        py,
        &format!(
            "Warning: lifecycle sink '{name}' did not return from one call within {}s; \
             its events are dropped until that call returns.\n",
            timeout.as_secs_f64()
        ),
    ));
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
    fn await_bound(&self, py: Python<'_>) -> PyResult<bool> {
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

    fn run_dispatch(&self, py: Python<'_>) -> PyResult<()> {
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
    fn dispatch_round(&self, py: Python<'_>) -> PyResult<bool> {
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
        let blocked: Vec<String> = std::mem::take(&mut state.newly_blocked);
        drop(state);
        for name in blocked {
            warn_blocked(py, &name, shared.invocation_timeout);
        }
        if notify {
            shared.notify_all(py)?;
        }
        if !stopped {
            shared.wait(py, POLL_SECONDS)?;
        }
        Ok(!stopped)
    }

    /// Every `(token, sanitized error type)` delivery threads reported since the last round.
    fn reported(&self, py: Python<'_>) -> PyResult<Vec<(u64, Option<String>)>> {
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

    fn next_notification(&self, py: Python<'_>) -> PyResult<Notification> {
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
            shared.wait(py, POLL_SECONDS)?;
        }
    }

    fn run_notifications(&self, py: Python<'_>) -> PyResult<()> {
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
    fn await_interval(&self, py: Python<'_>) -> PyResult<bool> {
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

    fn run_health(&self, py: Python<'_>) -> PyResult<()> {
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
    fn __call__(&self, py: Python<'_>) -> PyResult<()> {
        match self.role {
            WorkerRole::Dispatch => self.run_dispatch(py),
            WorkerRole::Notify => self.run_notifications(py),
            WorkerRole::Health => self.run_health(py),
        }
    }
}
