//! Public records of build progress.

use crate::boundary::main::imported::imported;
use crate::boundary::main::within::within;
use crate::build_progress::_helpers::progress::context_variable;
use crate::build_progress::_helpers::progress::persist;
use crate::build_progress::_helpers::progress::progress_event;
use crate::build_progress::_helpers::progress::shared_coordinator;
use crate::build_progress::_helpers::progress::terminal_request;
use crate::build_progress::_helpers::progress::terminal_stream;
use crate::build_progress::_helpers::terminal::claim;
use crate::build_progress::_helpers::terminal::flush;
use crate::build_progress::_helpers::terminal::join_spinner;
use crate::build_progress::_helpers::terminal::release;
use crate::build_progress::_helpers::terminal::shared_lines;
use crate::build_progress::_helpers::terminal::start_spinner;
use crate::build_progress::_helpers::terminal::terminal_columns;
use crate::build_progress::_helpers::terminal::tty_flag;
use crate::build_progress::_helpers::terminal::write;
use crate::build_progress::_helpers::transient_lines::ActiveLine;
use crate::build_progress::_helpers::transient_status::PREFLIGHT_COMPLETION_PREFIX;
use crate::build_progress::_helpers::transient_status::StatusState;
use crate::build_progress::_helpers::transient_status::progress_spinners_disabled;
use crate::build_progress::constants::CLEAR_LINE;
use crate::build_progress::constants::HIDE_CURSOR;
use crate::build_progress::constants::SHOW_CURSOR;
use crate::build_progress::constants::SPINNER_FRAMES;
use crate::build_progress::constants::SPINNER_TICK_SECONDS;
use crate::records::main::fields::field::field as native_field;
use crate::records::main::fields::mapping_items::mapping_items;
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::Bound;
use pyo3::prelude::Py;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyAnyMethods;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::pyclass;
use pyo3::pymethods;
use pyo3::types::PyDict;
use sqlbuild_core::cli_text::models::CliStyle;
use sqlbuild_core::progress::main::claim_progress_terminal::claim_progress_terminal;
use sqlbuild_core::progress::main::close_progress_projection::close_progress_projection;
use sqlbuild_core::progress::main::completion_phase_line::completion_phase_line;
use sqlbuild_core::progress::main::consume_progress_event::consume_progress_event;
use sqlbuild_core::progress::main::format_status_spinner_line::format_status_spinner_line;
use sqlbuild_core::progress::main::progress_terminal_duration::progress_terminal_duration;
use sqlbuild_core::progress::models::ProgressEvent;
use sqlbuild_core::progress::models::ProgressProjection;
use sqlbuild_core::progress::models::ProgressSettings;
use sqlbuild_core::progress::models::StatementSummary;
use sqlbuild_core::terminal_events::models::TerminalRequest;
use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::MutexGuard;
use std::sync::PoisonError;

/// Project canonical starts and correlate enriched terminal presentation.
#[pyclass(module = "sqlbuild._native", frozen)]
#[derive(Debug)]
pub struct NativeProgressProjector {
    pub(crate) state: Mutex<ProgressProjection>,
    pub(crate) stream: Py<PyAny>,
    pub(crate) coordinator: Py<PyAny>,
    /// The lines held back instead of written, while something holds them.
    pub(crate) held: Mutex<Option<Vec<String>>>,
    /// Each resource's lines held back until its result is known, while resources are held.
    pub(crate) held_by_resource: Mutex<Option<HeldResourceLines>>,
}

/// Lines held back per resource, in the order resources first wrote one.
#[derive(Debug, Default)]
pub(crate) struct HeldResourceLines {
    pub(crate) order: Vec<String>,
    pub(crate) lines: HashMap<String, Vec<String>>,
}

impl NativeProgressProjector {
    pub(crate) fn held(&self) -> MutexGuard<'_, Option<Vec<String>>> {
        self.held.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Whether lines are being held back instead of written.
    pub fn is_holding(&self) -> bool {
        self.held().is_some()
    }

    /// Hold back every line from now on instead of writing it.
    pub fn hold_lines(&self) {
        *self.held() = Some(Vec::new());
    }

    /// Replace the lines held back so far with `lines`, returning them; nothing while not holding.
    pub fn exchange_held_lines(&self, lines: Vec<String>) -> Vec<String> {
        match self.held().as_mut() {
            Some(held) => std::mem::replace(held, lines),
            None => Vec::new(),
        }
    }

    /// Write lines as they come again, returning those held back since `hold_lines`.
    pub fn release_lines(&self) -> Vec<String> {
        self.held().take().unwrap_or_default()
    }

    /// Write lines held back earlier, as they would have been written.
    pub fn write_held(&self, py: Python<'_>, lines: &[String]) -> PyResult<()> {
        let coordinator: &Bound<'_, PyAny> = self.coordinator.bind(py);
        for line in lines {
            persist(coordinator, self.stream.bind(py), line)?;
        }
        Ok(())
    }

    fn held_by_resource(&self) -> MutexGuard<'_, Option<HeldResourceLines>> {
        self.held_by_resource
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    /// Hold back each resource's lines from now on until `release_resource` or `discard_resource`.
    pub fn hold_resources(&self) {
        *self.held_by_resource() = Some(HeldResourceLines::default());
    }

    /// Write the lines held back for `resource_id`, as they would have been written.
    pub fn release_resource(&self, py: Python<'_>, resource_id: &str) -> PyResult<()> {
        let lines: Vec<String> = match self.held_by_resource().as_mut() {
            Some(held) => held.lines.remove(resource_id).unwrap_or_default(),
            None => Vec::new(),
        };
        self.write_held(py, &lines)
    }

    /// Drop the lines held back for `resource_id`, and never render that attempt's terminal.
    pub fn discard_resource(&self, resource_id: &str) -> PyResult<()> {
        if let Some(held) = self.held_by_resource().as_mut() {
            held.lines.remove(resource_id);
        }
        self.locked()?.forget_resource_attempts(resource_id);
        Ok(())
    }

    /// Never render the pending terminals of the resources reported under `resource_name`.
    pub fn forget_resource_named(&self, resource_name: &str) -> PyResult<()> {
        self.locked()?.forget_resource_named(resource_name);
        Ok(())
    }

    /// Write lines as they come again, first writing every resource's lines still held back.
    pub fn stop_holding_resources(&self, py: Python<'_>) -> PyResult<()> {
        let Some(mut held) = self.held_by_resource().take() else {
            return Ok(());
        };
        for resource_id in std::mem::take(&mut held.order) {
            let lines: Vec<String> = held.lines.remove(&resource_id).unwrap_or_default();
            self.write_held(py, &lines)?;
        }
        Ok(())
    }

    pub(crate) fn locked(&self) -> PyResult<MutexGuard<'_, ProgressProjection>> {
        self.state
            .lock()
            .map_err(|_| PyRuntimeError::new_err("native progress projector lock is poisoned"))
    }

    /// Run `update` and write its lines while holding the shared terminal line lock.
    pub(crate) fn write_with<T>(
        &self,
        py: Python<'_>,
        update: impl FnOnce(ProgressProjection) -> (ProgressProjection, Vec<String>, T),
    ) -> PyResult<T> {
        self.write_for(py, None, update)
    }

    /// `write_with` for lines about `resource_id`, which wait while resources are held.
    pub(crate) fn write_for<T>(
        &self,
        py: Python<'_>,
        resource_id: Option<&str>,
        update: impl FnOnce(ProgressProjection) -> (ProgressProjection, Vec<String>, T),
    ) -> PyResult<T> {
        let coordinator: &Bound<'_, PyAny> = self.coordinator.bind(py);
        let lock: Bound<'_, PyAny> = native_field(coordinator, "lock")?;
        lock.call_method0("acquire")?;
        let result: PyResult<T> = (|| {
            let (lines, value) = {
                let mut state: MutexGuard<'_, ProgressProjection> = self.locked()?;
                let (projection, lines, value) = update(std::mem::take(&mut *state));
                *state = projection;
                (lines, value)
            };
            for line in lines {
                let text: String = format!("{line}\n");
                if let (Some(resource_id), Some(held)) =
                    (resource_id, self.held_by_resource().as_mut())
                {
                    if !held.lines.contains_key(resource_id) {
                        held.order.push(resource_id.to_owned());
                    }
                    held.lines
                        .entry(resource_id.to_owned())
                        .or_default()
                        .push(text);
                    continue;
                }
                if let Some(held) = self.held().as_mut() {
                    held.push(text);
                    continue;
                }
                persist(coordinator, self.stream.bind(py), &text)?;
            }
            Ok(value)
        })();
        lock.call_method0("release")?;
        result
    }

    /// The warehouse statements this invocation finished so far.
    pub fn statement_summary(&self) -> PyResult<StatementSummary> {
        Ok(self.locked()?.statement_summary().clone())
    }
}

#[pymethods]
impl NativeProgressProjector {
    #[new]
    #[pyo3(signature = (*, stream, use_color, debug = false))]
    pub fn new(
        py: Python<'_>,
        stream: Bound<'_, PyAny>,
        use_color: bool,
        debug: bool,
    ) -> PyResult<Self> {
        let coordinator: Bound<'_, PyAny> = shared_coordinator(py)?;
        Ok(Self {
            state: Mutex::new(ProgressProjection::new(ProgressSettings {
                use_color,
                debug,
                is_tty: terminal_stream(&stream)?,
            })),
            stream: stream.unbind(),
            coordinator: coordinator.unbind(),
            held: Mutex::new(None),
            held_by_resource: Mutex::new(None),
        })
    }

    /// Install this projector for presentation adapters in the invocation context.
    pub(crate) fn install<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        context_variable(slf.py())?.call_method1("set", (slf,))
    }

    /// Restore the previous invocation projector.
    pub(crate) fn restore(&self, py: Python<'_>, token: &Bound<'_, PyAny>) -> PyResult<()> {
        context_variable(py)?.call_method1("reset", (token,))?;
        Ok(())
    }

    /// Add safe display order metadata without changing canonical facts.
    #[pyo3(signature = (*, ordinals, total))]
    pub(crate) fn configure_resources(
        &self,
        ordinals: &Bound<'_, PyAny>,
        total: i64,
    ) -> PyResult<()> {
        let mut entries: Vec<(String, i64)> = Vec::new();
        for item in mapping_items(ordinals)?.try_iter()? {
            entries.push(item?.extract()?);
        }
        self.locked()?.configure_resources(entries, total);
        Ok(())
    }

    /// Declare that an existing result callback will enrich this resource terminal.
    #[pyo3(signature = (*, resource_name))]
    pub(crate) fn expect_resource_enrichment(&self, resource_name: &str) -> PyResult<()> {
        self.locked()?.expect_resource_enrichment(resource_name);
        Ok(())
    }

    /// Print no start or statement failure rows for resources a result callback renders.
    pub(crate) fn quiet_enriched_resources(&self) -> PyResult<()> {
        self.locked()?.quiet_enriched_resources();
        Ok(())
    }

    /// The phase and kind of the statement that failed in `resource_name`, if one did.
    #[pyo3(signature = (*, resource_name))]
    pub fn failed_statement(&self, resource_name: &str) -> PyResult<Option<(String, String)>> {
        Ok(self.locked()?.failed_statement(resource_name))
    }

    /// Declare that this resource never starts, so it shows only its terminal row.
    #[pyo3(signature = (*, resource_name))]
    pub(crate) fn expect_no_start(&self, resource_name: &str) -> PyResult<()> {
        self.locked()?.expect_no_start(resource_name);
        Ok(())
    }

    /// Record how one audit attempt ended, so its terminal row shows the outcome.
    #[pyo3(signature = (*, resource_attempt_id, outcome))]
    pub(crate) fn record_audit_outcome(
        &self,
        resource_attempt_id: &str,
        outcome: &str,
    ) -> PyResult<()> {
        self.locked()?
            .record_audit_outcome(resource_attempt_id, outcome);
        Ok(())
    }

    /// Return whether a visible canonical operation currently owns progress.
    #[pyo3(name = "is_operation_active", signature = (*, operation_name))]
    pub fn operation_active(&self, operation_name: &str) -> PyResult<bool> {
        Ok(self.locked()?.is_operation_active(operation_name))
    }

    /// Consume one known lifecycle fact idempotently.
    pub(crate) fn consume(&self, py: Python<'_>, event: &Bound<'_, PyAny>) -> PyResult<()> {
        let event: ProgressEvent = progress_event(event)?;
        self.write_for(py, event.resource_id.as_deref(), |projection| {
            let (projection, lines) = consume_progress_event(projection, &event);
            (projection, lines, ())
        })
    }

    /// Claim the targeted or latest terminal for one final resource result.
    #[pyo3(signature = (**options))]
    pub(crate) fn consume_resource_terminal(
        &self,
        py: Python<'_>,
        options: Option<Bound<'_, PyDict>>,
    ) -> PyResult<Option<f64>> {
        let request: TerminalRequest = terminal_request(py, "consume_resource_terminal", options)?;
        self.write_with(py, |projection| {
            claim_progress_terminal(projection, &request)
        })
    }

    /// Return a terminal duration without claiming its presentation.
    #[pyo3(signature = (**options))]
    pub(crate) fn resource_terminal_duration(
        &self,
        py: Python<'_>,
        options: Option<Bound<'_, PyDict>>,
    ) -> PyResult<Option<f64>> {
        let request: TerminalRequest = terminal_request(py, "resource_terminal_duration", options)?;
        Ok(progress_terminal_duration(&*self.locked()?, &request))
    }

    /// Render resource terminals no result callback claimed.
    pub(crate) fn close(&self, py: Python<'_>) -> PyResult<()> {
        self.write_with(py, |projection| {
            let (projection, lines) = close_progress_projection(projection);
            (projection, lines, ())
        })
    }
}

/// Keep persistent terminal lines from landing on a live spinner or status line.
#[pyclass(module = "sqlbuild._native", frozen)]
#[derive(Debug)]
pub struct TransientLineCoordinator {
    /// The re-entrant lock every writer holds while it draws.
    #[pyo3(get)]
    pub lock: Py<PyAny>,
    pub(crate) active: Mutex<Option<ActiveLine>>,
}

impl TransientLineCoordinator {
    pub(crate) fn active(&self) -> MutexGuard<'_, Option<ActiveLine>> {
        self.active.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Run `body` while holding the coordinator lock.
    pub(crate) fn locked<T>(
        &self,
        py: Python<'_>,
        body: impl FnOnce() -> PyResult<T>,
    ) -> PyResult<T> {
        let lock: &Bound<'_, PyAny> = self.lock.bind(py);
        lock.call_method0("acquire")?;
        let result: PyResult<T> = body();
        lock.call_method0("release")?;
        result
    }

    /// The owner whose line a write to `stream` would land on.
    pub(crate) fn interrupted_owner<'py>(
        &self,
        stream: &Bound<'py, PyAny>,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        let py: Python<'py> = stream.py();
        let Some((owner_stream, owner)) = self
            .active()
            .as_ref()
            .map(|(owner_stream, owner)| (owner_stream.bind(py).clone(), owner.bind(py).clone()))
        else {
            return Ok(None);
        };
        if owner_stream.is(stream) || (tty_flag(&owner_stream)? && tty_flag(stream)?) {
            return Ok(Some(owner));
        }
        Ok(None)
    }
}

#[pymethods]
impl TransientLineCoordinator {
    #[new]
    pub(crate) fn new(py: Python<'_>) -> PyResult<Self> {
        Ok(Self {
            lock: imported(py, "threading", "RLock")?.call0()?.unbind(),
            active: Mutex::new(None),
        })
    }

    /// Record the owner of the transient line currently drawn on a terminal stream.
    #[pyo3(signature = (*, stream, owner))]
    pub(crate) fn claim(
        &self,
        stream: &Bound<'_, PyAny>,
        owner: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        self.locked(stream.py(), || {
            *self.active() = Some((stream.clone().unbind(), owner.clone().unbind()));
            Ok(())
        })
    }

    /// Forget a transient owner once its line is no longer drawn.
    #[pyo3(signature = (*, owner))]
    pub(crate) fn release(&self, owner: &Bound<'_, PyAny>) -> PyResult<()> {
        self.locked(owner.py(), || {
            let mut active = self.active();
            if active
                .as_ref()
                .is_some_and(|(_, current)| current.is(owner))
            {
                *active = None;
            }
            Ok(())
        })
    }

    /// Erase and forget any live transient line, so nothing redraws it afterwards.
    pub(crate) fn clear_active(&self, py: Python<'_>) -> PyResult<()> {
        self.locked(py, || {
            let taken: Option<ActiveLine> = self.active().take();
            if let Some((_, owner)) = taken {
                owner.bind(py).call_method0("clear_transient_line")?;
            }
            Ok(())
        })
    }

    /// Write lasting text, clearing and then redrawing any live transient line.
    #[pyo3(signature = (*, stream, text))]
    pub(crate) fn write_persistent(
        &self,
        stream: &Bound<'_, PyAny>,
        text: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        self.locked(stream.py(), || {
            let owner: Option<Bound<'_, PyAny>> = self.interrupted_owner(stream)?;
            if let Some(owner) = &owner {
                owner.call_method0("clear_transient_line")?;
            }
            stream.call_method1("write", (text,))?;
            stream.call_method0("flush")?;
            if let Some(owner) = &owner {
                owner.call_method0("redraw_transient_line")?;
            }
            Ok(())
        })
    }
}

/// Render one transient status line with optional spinner updates.
#[pyclass(module = "sqlbuild._native", frozen)]
#[derive(Debug)]
pub struct TransientStatusReporter {
    pub(crate) stream: Py<PyAny>,
    pub(crate) lines: Py<PyAny>,
    pub(crate) lock: Py<PyAny>,
    pub(crate) style: CliStyle,
    pub(crate) enabled: bool,
    pub(crate) state: Mutex<StatusState>,
}

impl TransientStatusReporter {
    pub(crate) fn state(&self) -> MutexGuard<'_, StatusState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub(crate) fn locked<T>(
        &self,
        py: Python<'_>,
        body: impl FnOnce() -> PyResult<T>,
    ) -> PyResult<T> {
        within(self.lock.bind(py), |_| body())
    }

    pub fn write_persistent(&self, py: Python<'_>, text: &str) -> PyResult<()> {
        persist(self.lines.bind(py), self.stream.bind(py), text)
    }

    pub(crate) fn write_message(
        &self,
        py: Python<'_>,
        message: &str,
        dim_output: bool,
    ) -> PyResult<()> {
        let formatted: String = if dim_output {
            self.style.muted(message)
        } else {
            message.to_owned()
        };
        self.write_persistent(py, &format!("{formatted}\n"))
    }

    pub(crate) fn draw(&self, py: Python<'_>) -> PyResult<()> {
        let (message, frame) = {
            let mut state = self.state();
            let Some(message) = state.message.clone().filter(|_| state.drawn) else {
                return Ok(());
            };
            let frame: usize = state.frame;
            state.frame = (frame + 1) % SPINNER_FRAMES.len();
            (message, frame)
        };
        let line: String = format_status_spinner_line(
            SPINNER_FRAMES[frame],
            &message,
            terminal_columns(py)?,
            self.style,
        );
        let stream: &Bound<'_, PyAny> = self.stream.bind(py);
        write(stream, &format!("{CLEAR_LINE}{line}"))?;
        flush(stream)
    }

    pub(crate) fn enter_status(slf: &Bound<'_, Self>) -> PyResult<()> {
        let py: Python<'_> = slf.py();
        let reporter: &Self = slf.get();
        let register_hook: bool =
            !std::mem::replace(&mut reporter.state().exit_hook_registered, true);
        if register_hook {
            imported(py, "atexit", "register")?.call1((native_field(slf, "close")?,))?;
        }
        let stream: &Bound<'_, PyAny> = reporter.stream.bind(py);
        write(stream, HIDE_CURSOR)?;
        reporter.state().drawn = true;
        reporter.draw(py)?;
        let (event, thread) = start_spinner(slf.as_any())?;
        let mut state = reporter.state();
        state.stop_event = Some(event.unbind());
        state.threads.push(thread.unbind());
        Ok(())
    }

    pub(crate) fn close_status(slf: &Bound<'_, Self>) -> PyResult<()> {
        let py: Python<'_> = slf.py();
        let reporter: &Self = slf.get();
        reporter.locked(py, || {
            release(reporter.lines.bind(py), slf.as_any())?;
            reporter.exit_status(py)?;
            let mut state = reporter.state();
            state.message = None;
            state.active = false;
            Ok(())
        })
    }

    pub(crate) fn exit_status(&self, py: Python<'_>) -> PyResult<()> {
        let (drawn, event) = {
            let mut state = self.state();
            (std::mem::take(&mut state.drawn), state.stop_event.take())
        };
        if let Some(event) = event {
            event.bind(py).call_method0("set")?;
        }
        if drawn {
            let stream: &Bound<'_, PyAny> = self.stream.bind(py);
            write(stream, &format!("{CLEAR_LINE}{SHOW_CURSOR}"))?;
            flush(stream)?;
        }
        Ok(())
    }
}

#[pymethods]
impl TransientStatusReporter {
    #[new]
    #[pyo3(signature = (*, stream, use_color = false, enabled = true))]
    pub fn new(
        py: Python<'_>,
        stream: &Bound<'_, PyAny>,
        use_color: bool,
        enabled: bool,
    ) -> PyResult<Self> {
        let lines: Bound<'_, PyAny> = shared_lines(py)?;
        Ok(Self {
            enabled: enabled && !progress_spinners_disabled() && tty_flag(stream)?,
            stream: stream.clone().unbind(),
            lock: native_field(&lines, "lock")?.unbind(),
            lines: lines.unbind(),
            style: CliStyle { use_color },
            state: Mutex::new(StatusState::default()),
        })
    }

    pub fn start(slf: &Bound<'_, Self>, message: String) -> PyResult<()> {
        let py: Python<'_> = slf.py();
        let reporter: &Self = slf.get();
        if reporter.enabled {
            return reporter.locked(py, || {
                Self::close_status(slf)?;
                reporter.state().message = Some(message);
                Self::enter_status(slf)?;
                claim(
                    reporter.lines.bind(py),
                    reporter.stream.bind(py),
                    slf.as_any(),
                )?;
                reporter.state().active = true;
                Ok(())
            });
        }
        reporter.state().active = true;
        reporter.write_message(py, &message, true)
    }

    pub fn update(&self, py: Python<'_>, message: String) -> PyResult<()> {
        if self.enabled {
            let updated: bool = self.locked(py, || {
                let drawn: bool = {
                    let mut state = self.state();
                    if state.message.is_none() {
                        return Ok(false);
                    }
                    state.message = Some(message.clone());
                    state.drawn
                };
                if drawn {
                    self.draw(py)?;
                }
                Ok(true)
            })?;
            if updated {
                return Ok(());
            }
        }
        self.write_message(py, &message, true)
    }

    #[pyo3(signature = (*, message, blank_line_after = false))]
    pub fn complete(slf: &Bound<'_, Self>, message: &str, blank_line_after: bool) -> PyResult<()> {
        let py: Python<'_> = slf.py();
        Self::close(slf)?;
        match completion_phase_line(message, slf.get().style) {
            Some(line) => slf.get().write_persistent(py, &format!("{line}\n"))?,
            None => slf.get().write_message(py, message, true)?,
        }
        if blank_line_after {
            slf.get().write_blank_line(py)?;
        }
        Ok(())
    }

    /// Complete with a pre-styled message written verbatim.
    #[pyo3(signature = (*, message, blank_line_after = false))]
    pub fn complete_styled(
        slf: &Bound<'_, Self>,
        message: &str,
        blank_line_after: bool,
    ) -> PyResult<()> {
        Self::close(slf)?;
        let blank: &str = if blank_line_after { "\n" } else { "" };
        slf.get()
            .write_persistent(slf.py(), &format!("{message}\n{blank}"))
    }

    pub fn error(slf: &Bound<'_, Self>, message: &str) -> PyResult<()> {
        Self::close(slf)?;
        slf.get().write_message(slf.py(), message, false)
    }

    pub(crate) fn close(slf: &Bound<'_, Self>) -> PyResult<()> {
        Self::close_status(slf)?;
        let threads: Vec<Py<PyAny>> = std::mem::take(&mut slf.get().state().threads);
        for thread in threads {
            join_spinner(thread.bind(slf.py()))?;
        }
        Ok(())
    }

    /// Erase the live status so a persistent line can be written in its place; the spinner idles.
    pub(crate) fn clear_transient_line(&self, py: Python<'_>) -> PyResult<()> {
        if std::mem::take(&mut self.state().drawn) {
            let stream: &Bound<'_, PyAny> = self.stream.bind(py);
            write(stream, &format!("{CLEAR_LINE}{SHOW_CURSOR}"))?;
            flush(stream)?;
        }
        Ok(())
    }

    /// Draw the live status again below persistent output, reusing its running spinner.
    pub(crate) fn redraw_transient_line(slf: &Bound<'_, Self>) -> PyResult<()> {
        let reporter: &Self = slf.get();
        let (redraw, spinning) = {
            let state = reporter.state();
            (
                state.message.is_some() && !state.drawn,
                state.stop_event.is_some(),
            )
        };
        if !redraw {
            return Ok(());
        }
        if !spinning {
            return Self::enter_status(slf);
        }
        write(reporter.stream.bind(slf.py()), HIDE_CURSOR)?;
        reporter.state().drawn = true;
        reporter.draw(slf.py())
    }

    /// Render one preflight progress update.
    pub(crate) fn report_preflight_progress(
        slf: &Bound<'_, Self>,
        message: String,
    ) -> PyResult<()> {
        if message.starts_with(PREFLIGHT_COMPLETION_PREFIX) {
            return Self::complete(slf, &message, true);
        }
        if !slf.get().state().active {
            return Self::start(slf, message);
        }
        slf.get().update(slf.py(), message)
    }

    pub(crate) fn write_blank_line(&self, py: Python<'_>) -> PyResult<()> {
        self.write_persistent(py, "\n")
    }

    pub(crate) fn _spin_until_stopped(
        &self,
        py: Python<'_>,
        stop_event: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        while !stop_event
            .call_method1("wait", (SPINNER_TICK_SECONDS,))?
            .is_truthy()?
        {
            self.locked(py, || {
                if stop_event.call_method0("is_set")?.is_truthy()? {
                    return Ok(());
                }
                self.draw(py)
            })?;
        }
        Ok(())
    }
}
