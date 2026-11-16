//! Warehouse connection and planning phase progress reporters.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, PoisonError};
use std::time::Instant;

use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods};
use pyo3::{pyclass, pymethods};
use sqlbuild_core::cli_text::models::CliStyle;
use sqlbuild_core::progress::main::connection_phase_line::connection_phase_line;
use sqlbuild_core::progress::main::connection_start_message::connection_start_message;
use sqlbuild_core::progress::main::is_planning_completion_message::is_planning_completion_message;
use sqlbuild_core::progress::main::is_planning_failure_message::is_planning_failure_message;

use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::build_progress::main::progress::current_projector::current_projector;
use sqlbuild_python_base::build_progress::main::progress::projector_operation_active::projector_operation_active;
use sqlbuild_python_base::build_progress::main::progress::write_persistent::write_persistent;
use sqlbuild_python_base::build_progress::main::transient_status::status_reporter::status_reporter;
use sqlbuild_python_base::build_progress::models::NativeProgressProjector;

const CONNECTION_OPTIONS: (&[&str], &[&str]) = (
    &["adapter_name", "stream"],
    &[
        "blank_line_before_start",
        "blank_line_after_complete",
        "use_color",
    ],
);
const PROJECT_COMPILE_OPERATION: &str = "project_compile";

/// Render simple connection start, success, and failure messages.
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct ConnectionProgressReporter {
    adapter_name: String,
    stream: Py<PyAny>,
    blank_line_before_start: bool,
    blank_line_after_complete: bool,
    style: CliStyle,
}

impl ConnectionProgressReporter {
    fn write(&self, py: Python<'_>, message: &str) -> PyResult<()> {
        write_persistent(py, self.stream.bind(py), &format!("{message}\n"))
    }
}

#[pymethods]
impl ConnectionProgressReporter {
    #[new]
    #[pyo3(signature = (**options))]
    pub(crate) fn new(py: Python<'_>, options: Option<Bound<'_, PyDict>>) -> PyResult<Self> {
        let arguments: KeywordArguments<'_> = KeywordArguments::read(
            py,
            "ConnectionProgressReporter",
            options,
            CONNECTION_OPTIONS,
        )?;
        Ok(Self {
            adapter_name: arguments.value("adapter_name")?.extract()?,
            stream: arguments.value("stream")?.unbind(),
            blank_line_before_start: arguments.flag("blank_line_before_start")?,
            blank_line_after_complete: arguments.flag("blank_line_after_complete")?,
            style: CliStyle {
                use_color: arguments.flag("use_color")?,
            },
        })
    }

    fn on_connection_start(&self, py: Python<'_>, connection_count: i64) -> PyResult<()> {
        if self.blank_line_before_start {
            self.write(py, "")?;
        }
        let message: String = connection_start_message(&self.adapter_name, connection_count);
        self.write(py, &self.style.muted(&message))
    }

    #[pyo3(signature = (*, connection_count, elapsed_seconds))]
    fn on_connection_complete(
        &self,
        py: Python<'_>,
        connection_count: i64,
        elapsed_seconds: f64,
    ) -> PyResult<()> {
        let _ = connection_count;
        let line: String =
            connection_phase_line(self.style, true, (&self.adapter_name, elapsed_seconds));
        self.write(py, &line)?;
        if self.blank_line_after_complete {
            self.write(py, "")?;
        }
        Ok(())
    }

    #[pyo3(signature = (*, connection_count, elapsed_seconds))]
    fn on_connection_error(
        &self,
        py: Python<'_>,
        connection_count: i64,
        elapsed_seconds: f64,
    ) -> PyResult<()> {
        let _ = connection_count;
        let line: String =
            connection_phase_line(self.style, false, (&self.adapter_name, elapsed_seconds));
        self.write(py, &line)
    }
}

/// Render simple planning phase progress messages.
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct PlanningProgressReporter {
    status: Py<PyAny>,
    active: AtomicBool,
    projector: Option<Py<NativeProgressProjector>>,
}

impl PlanningProgressReporter {
    fn call_status(&self, py: Python<'_>, method: &str, message: &str) -> PyResult<()> {
        self.status.bind(py).call_method1(method, (message,))?;
        Ok(())
    }

    fn call_status_keyword(&self, py: Python<'_>, method: &str, message: &str) -> PyResult<()> {
        let keywords: Bound<'_, PyDict> = PyDict::new(py);
        keywords.set_item("message", message)?;
        self.status
            .bind(py)
            .call_method(method, (), Some(&keywords))?;
        Ok(())
    }
}

#[pymethods]
impl PlanningProgressReporter {
    #[new]
    #[pyo3(signature = (*, stream, use_color = false))]
    pub(crate) fn new(
        py: Python<'_>,
        stream: &Bound<'_, PyAny>,
        use_color: bool,
    ) -> PyResult<Self> {
        let status: Bound<'_, PyAny> = status_reporter(stream, use_color)?;
        Ok(Self {
            status: status.unbind(),
            active: AtomicBool::new(false),
            projector: current_projector(py)?,
        })
    }

    pub(crate) fn on_progress(&self, py: Python<'_>, message: &str) -> PyResult<()> {
        if let Some(projector) = &self.projector
            && projector_operation_active(projector.get(), PROJECT_COMPILE_OPERATION)?
        {
            return Ok(());
        }
        if is_planning_completion_message(message) {
            return self.complete(py, message);
        }
        if is_planning_failure_message(message) {
            return self.error(py, message);
        }
        if !self.active.load(Ordering::SeqCst) {
            return self.start(py, message);
        }
        self.update(py, message)
    }

    pub(crate) fn start(&self, py: Python<'_>, message: &str) -> PyResult<()> {
        self.call_status(py, "start", message)?;
        self.active.store(true, Ordering::SeqCst);
        Ok(())
    }

    fn update(&self, py: Python<'_>, message: &str) -> PyResult<()> {
        if !self.active.load(Ordering::SeqCst) {
            return self.start(py, message);
        }
        self.call_status(py, "update", message)
    }

    pub(crate) fn complete(&self, py: Python<'_>, message: &str) -> PyResult<()> {
        self.call_status_keyword(py, "complete", message)?;
        self.active.store(false, Ordering::SeqCst);
        Ok(())
    }

    /// Complete with a pre-styled message written verbatim.
    pub(crate) fn complete_styled(&self, py: Python<'_>, message: &str) -> PyResult<()> {
        self.call_status_keyword(py, "complete_styled", message)?;
        self.active.store(false, Ordering::SeqCst);
        Ok(())
    }

    /// Close active progress and render a non-success terminal message.
    fn error(&self, py: Python<'_>, message: &str) -> PyResult<()> {
        self.call_status(py, "error", message)?;
        self.active.store(false, Ordering::SeqCst);
        Ok(())
    }

    #[pyo3(signature = (*, blank_line_after = false))]
    pub(crate) fn finish(&self, py: Python<'_>, blank_line_after: bool) -> PyResult<()> {
        self.status.bind(py).call_method0("close")?;
        self.active.store(false, Ordering::SeqCst);
        if blank_line_after {
            self.status.bind(py).call_method0("write_blank_line")?;
        }
        Ok(())
    }
}

/// Render timed `sqb dbt init` phase progress.
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct DbtInitProgressReporter {
    stream: Py<PyAny>,
    status: Py<PyAny>,
    started: AtomicBool,
    phase_start: Mutex<Option<Instant>>,
}

#[pymethods]
impl DbtInitProgressReporter {
    #[new]
    #[pyo3(signature = (*, stream, use_color))]
    pub(crate) fn new(stream: &Bound<'_, PyAny>, use_color: bool) -> PyResult<Self> {
        Ok(Self {
            stream: stream.clone().unbind(),
            status: status_reporter(stream, use_color)?.unbind(),
            started: AtomicBool::new(false),
            phase_start: Mutex::new(None),
        })
    }

    fn start(&self, py: Python<'_>, message: &str) -> PyResult<()> {
        if !self.started.load(Ordering::SeqCst) {
            let stream: &Bound<'_, PyAny> = self.stream.bind(py);
            stream.call_method1("write", ("\n",))?;
            stream.call_method0("flush")?;
            self.started.store(true, Ordering::SeqCst);
        }
        *self
            .phase_start
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(Instant::now());
        self.status.bind(py).call_method1("start", (message,))?;
        Ok(())
    }

    fn complete(&self, py: Python<'_>, message: &str) -> PyResult<()> {
        let phase_start: Option<Instant> = self
            .phase_start
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        let elapsed_seconds: f64 = phase_start.map_or(0.0, |start| start.elapsed().as_secs_f64());
        let keywords: Bound<'_, PyDict> = PyDict::new(py);
        keywords.set_item("message", format!("{message} ({elapsed_seconds:.2}s)"))?;
        self.status
            .bind(py)
            .call_method("complete", (), Some(&keywords))?;
        Ok(())
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<ConnectionProgressReporter>()?;
    module.add_class::<DbtInitProgressReporter>()?;
    module.add_class::<PlanningProgressReporter>()
}
