//! Streamed janitor phase and action lines, as the executor's progress callbacks.

use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::Instant;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::{pyclass, pymethods};
use sqlbuild_core::cli_text::models::CliStyle;
use sqlbuild_core::janitor_text::main::janitor_events_unwritten_text::janitor_events_unwritten_text;
use sqlbuild_core::janitor_text::main::janitor_phase_complete_text::janitor_phase_complete_text;
use sqlbuild_core::janitor_text::main::janitor_phase_failed_text::janitor_phase_failed_text;
use sqlbuild_core::janitor_text::main::janitor_phase_flow_width::janitor_phase_flow_width;
use sqlbuild_core::janitor_text::main::janitor_phase_start_text::janitor_phase_start_text;
use sqlbuild_core::janitor_text::main::render_janitor_item_line::render_janitor_item_line;
use sqlbuild_core::janitor_text::models::{JanitorItemText, JanitorPhase, JanitorPlanText};

use sqlbuild_python_base::build_progress::main::terminal::tty_flag::tty_flag;
use sqlbuild_python_base::build_progress::main::transient_status::progress_spinners_disabled::progress_spinners_disabled;
use sqlbuild_python_base::build_progress::models::TransientStatusReporter;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// The running phase's start text, size, finished count, column width and start time.
struct ActivePhase {
    message: String,
    total: usize,
    finished: usize,
    flow_width: usize,
    started: Instant,
}

#[derive(Default)]
struct ProgressState {
    items: Vec<JanitorItemText>,
    active: Option<ActivePhase>,
}

/// Print each janitor phase and action as it finishes, and keep the finished actions.
#[pyclass(frozen)]
pub(crate) struct JanitorProgressPrinter {
    status: Py<TransientStatusReporter>,
    style: CliStyle,
    live: bool,
    plan: JanitorPlanText,
    state: Mutex<ProgressState>,
}

impl JanitorProgressPrinter {
    pub(crate) fn new(
        stream: &Bound<'_, PyAny>,
        style: CliStyle,
        plan: JanitorPlanText,
    ) -> PyResult<Self> {
        let py: Python<'_> = stream.py();
        Ok(Self {
            status: Py::new(
                py,
                TransientStatusReporter::new(py, stream, style.use_color, true)?,
            )?,
            style,
            live: tty_flag(stream)? && !progress_spinners_disabled(),
            plan,
            state: Mutex::new(ProgressState::default()),
        })
    }

    fn state(&self) -> MutexGuard<'_, ProgressState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Every action finished so far, in order.
    pub(crate) fn items(&self) -> Vec<JanitorItemText> {
        self.state().items.clone()
    }
}

fn phase(value: &str) -> PyResult<JanitorPhase> {
    JanitorPhase::from_value(value)
        .ok_or_else(|| PyValueError::new_err(format!("unknown janitor phase: {value}")))
}

#[pymethods]
impl JanitorProgressPrinter {
    #[pyo3(signature = (*, phase, total))]
    fn phase_started(&self, py: Python<'_>, phase: &str, total: usize) -> PyResult<()> {
        let phase: JanitorPhase = self::phase(phase)?;
        let message: String = janitor_phase_start_text(phase, total);
        self.state().active = Some(ActivePhase {
            message: message.clone(),
            total,
            finished: 0,
            flow_width: janitor_phase_flow_width(&self.plan, phase),
            started: Instant::now(),
        });
        TransientStatusReporter::start(self.status.bind(py), message)
    }

    /// Record and print one finished action; `item` is its finished `JanitorProgressItem`.
    #[pyo3(signature = (*, phase, item))]
    fn item_finished(&self, py: Python<'_>, phase: &str, item: &Bound<'_, PyAny>) -> PyResult<()> {
        let item: JanitorItemText = JanitorItemText {
            phase: self::phase(phase)?,
            action: native_field(item, "action")?.str()?.extract()?,
            name: native_field(item, "name")?.extract()?,
            destination: native_field(item, "destination")?.extract()?,
            error: native_field(item, "error")?.extract()?,
            duration_seconds: Some(native_field(item, "duration_seconds")?.extract()?),
        };
        let (line, progress) = {
            let mut state = self.state();
            let (index, total, width, message) = match state.active.as_mut() {
                Some(active) => {
                    active.finished += 1;
                    (
                        active.finished,
                        active.total,
                        active.flow_width,
                        active.message.clone(),
                    )
                }
                None => (1, 1, 0, String::new()),
            };
            let line: String = render_janitor_item_line((index, total), &item, width, self.style);
            state.items.push(item);
            (line, format!("{message} {index}/{total}"))
        };
        let status: &TransientStatusReporter = self.status.get();
        status.write_persistent(py, &format!("{line}\n"))?;
        if self.live {
            status.update(py, progress)?;
        }
        Ok(())
    }

    /// Warn that the audit events of `count` completed actions in `schemas` were not written.
    #[pyo3(signature = (*, count, schemas, error))]
    fn events_unwritten(
        &self,
        py: Python<'_>,
        count: usize,
        schemas: Vec<String>,
        error: &str,
    ) -> PyResult<()> {
        let line: String = janitor_events_unwritten_text((count, &schemas), error, self.style);
        self.status.get().write_persistent(py, &format!("{line}\n"))
    }

    #[pyo3(signature = (*, phase, failed))]
    fn phase_finished(&self, py: Python<'_>, phase: &str, failed: bool) -> PyResult<()> {
        let phase: JanitorPhase = self::phase(phase)?;
        let (active, items) = {
            let mut state = self.state();
            (state.active.take(), state.items.clone())
        };
        let (total, elapsed) = active.as_ref().map_or((0, 0.0), |active| {
            (active.total, active.started.elapsed().as_secs_f64())
        });
        let status: &Bound<'_, TransientStatusReporter> = self.status.bind(py);
        if failed {
            let completed: usize = items
                .iter()
                .filter(|item| item.phase == phase && item.error.is_none())
                .count();
            let message: String =
                janitor_phase_failed_text(phase, (completed, total), elapsed, self.style);
            return TransientStatusReporter::complete_styled(status, &message, false);
        }
        TransientStatusReporter::complete(
            status,
            &janitor_phase_complete_text(phase, &items, elapsed),
            false,
        )
    }
}
