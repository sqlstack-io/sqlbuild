//! Live and persistent progress lines of `sqb load`, as executor callbacks.

use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard, PoisonError};

use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyBool, PyDict, PyDictMethods, PyString};
use pyo3::{pyclass, pymethods};
use sqlbuild_core::cli_text::models::CliStyle;
use sqlbuild_core::load_command::main::render_load_result::render_load_result;
use sqlbuild_core::load_command::main::render_load_spinner_line::render_load_spinner_line;
use sqlbuild_core::load_command::models::{LoadResultText, LoadSpinnerRow, LoadStatus};
use sqlbuild_host::entries::models::AdapterHost;

use sqlbuild_python_base::build_progress::main::progress::current_native_progress_projector::current_native_progress_projector;
use sqlbuild_python_base::build_progress::main::terminal::start_spinner::start_spinner;
use sqlbuild_python_base::build_progress::main::transient_lines::shared_transient_line_coordinator::shared_transient_line_coordinator;
use sqlbuild_python_base::boundary::main::within::within;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

const LOADER_NODE_KEY: &str = "sqlbuild_loader_node";
const SPINNER_TICK_SECONDS: f64 = 0.1;
const SPINNER_JOIN_SECONDS: f64 = 0.2;
const CLEAR_LINE: &str = "\r\u{1b}[K";
const HIDE_CURSOR: &str = "\u{1b}[?25l";
const SHOW_CURSOR: &str = "\u{1b}[?25h";

fn optional_text(value: Bound<'_, PyAny>) -> PyResult<Option<String>> {
    if value.is_none() {
        return Ok(None);
    }
    value.extract().map(Some)
}

/// One executor `LoadExecutionResult` as the command renders it.
pub(crate) fn load_result_text(result: &Bound<'_, PyAny>) -> PyResult<LoadResultText> {
    let py: Python<'_> = result.py();
    let log_kind: Bound<'_, PyAny> =
        host_entry(py, AdapterHost::LifeCycleEventKind)?.getattr("LOG")?;
    let mut log_messages: Vec<String> = Vec::new();
    for event in native_field(result, "lifecycle_events")?.try_iter()? {
        let event: Bound<'_, PyAny> = event?;
        if native_field(&event, "kind")?.eq(&log_kind)? {
            log_messages.push(native_field(&event, "content")?.extract()?);
        }
    }
    let duration: Bound<'_, PyAny> = native_field(result, "duration_ms")?;
    Ok(LoadResultText {
        source_name: native_field(result, "source_name")?.extract()?,
        loader_name: native_field(result, "loader_name")?.str()?.extract()?,
        resource_kind: native_field(result, "resource_kind")?
            .getattr("value")?
            .extract()?,
        status: LoadStatus::from_value(
            &native_field(result, "status")?
                .getattr("value")?
                .extract::<String>()?,
        ),
        rows_loaded: native_field(result, "rows_loaded")?.extract()?,
        duration_ms: if duration.is_none() {
            None
        } else {
            Some(duration.extract()?)
        },
        log_messages,
        error_message: optional_text(native_field(result, "error_message")?)?,
        warning_messages: native_field(result, "warning_messages")?.extract()?,
    })
}

/// The source currently shown on the live row.
#[derive(Clone)]
struct ActiveSource {
    name: String,
    resource_kind: String,
}

/// Mutable reporter state; only held briefly and never across Python calls.
#[derive(Default)]
struct ReporterState {
    active: Option<ActiveSource>,
    message: String,
    frame: usize,
    line_active: bool,
    cursor_hidden: bool,
    stop_event: Option<Py<PyAny>>,
    thread: Option<Py<PyAny>>,
}

/// TTY-aware renderer for `sqb load` execution progress.
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct LoadProgressReporter {
    stream: Py<PyAny>,
    lines: Py<PyAny>,
    write_lock: Py<PyAny>,
    style: CliStyle,
    source_order: HashMap<String, i64>,
    total: i64,
    is_tty: bool,
    projector: Option<Py<PyAny>>,
    state: Mutex<ReporterState>,
}

impl LoadProgressReporter {
    fn state(&self) -> MutexGuard<'_, ReporterState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn write(&self, py: Python<'_>, text: &str) -> PyResult<()> {
        let stream: &Bound<'_, PyAny> = self.stream.bind(py);
        stream.call_method1("write", (text,))?;
        stream.call_method0("flush")?;
        Ok(())
    }

    fn locked<T>(&self, py: Python<'_>, body: impl FnOnce() -> PyResult<T>) -> PyResult<T> {
        within(self.write_lock.bind(py), |_| body())
    }

    fn write_spinner_line(&self, py: Python<'_>) -> PyResult<()> {
        self.locked(py, || {
            let (active, message, frame) = {
                let mut state = self.state();
                if !state.line_active {
                    return Ok(());
                }
                let Some(active) = state.active.clone() else {
                    return Ok(());
                };
                let frame: usize = state.frame;
                state.frame = (frame + 1) % 10;
                (active, state.message.clone(), frame)
            };
            let row: LoadSpinnerRow = LoadSpinnerRow {
                ordinal: self.source_order[&active.name],
                total: self.total,
                resource_kind: active.resource_kind,
                name: active.name,
                message,
            };
            self.write(py, &render_load_spinner_line(&row, frame, self.style))
        })
    }

    fn set_cursor(&self, py: Python<'_>, hidden: bool) -> PyResult<()> {
        if self.state().cursor_hidden == hidden {
            return Ok(());
        }
        self.locked(py, || {
            self.write(py, if hidden { HIDE_CURSOR } else { SHOW_CURSOR })
        })?;
        self.state().cursor_hidden = hidden;
        Ok(())
    }

    fn stop_spinner(&self, py: Python<'_>) -> PyResult<()> {
        let (event, thread) = {
            let mut state = self.state();
            (state.stop_event.take(), state.thread.take())
        };
        if let Some(event) = event {
            event.bind(py).call_method0("set")?;
        }
        if let Some(thread) = thread {
            let thread: &Bound<'_, PyAny> = thread.bind(py);
            if thread.call_method0("is_alive")?.is_truthy()? {
                let options: Bound<'_, PyDict> = PyDict::new(py);
                options.set_item("timeout", SPINNER_JOIN_SECONDS)?;
                thread.call_method("join", (), Some(&options))?;
            }
        }
        Ok(())
    }

    fn start_spinner(slf: &Bound<'_, Self>) -> PyResult<()> {
        let py: Python<'_> = slf.py();
        slf.get().stop_spinner(py)?;
        let (event, thread) = start_spinner(slf.as_any())?;
        let mut state = slf.get().state();
        state.stop_event = Some(event.unbind());
        state.thread = Some(thread.unbind());
        Ok(())
    }
}

#[pymethods]
impl LoadProgressReporter {
    #[new]
    #[pyo3(signature = (**options))]
    pub(crate) fn new(py: Python<'_>, options: Option<Bound<'_, PyDict>>) -> PyResult<Self> {
        let arguments: KeywordArguments<'_> = KeywordArguments::read(
            py,
            "LoadProgressReporter",
            options,
            (&["stream", "use_color", "source_order", "total_count"], &[]),
        )?;
        let stream: Bound<'_, PyAny> = arguments.value("stream")?;
        let source_order: HashMap<String, i64> = arguments.value("source_order")?.extract()?;
        let total_count: i64 = arguments.value("total_count")?.extract()?;
        let lines: Bound<'_, PyAny> = shared_transient_line_coordinator(py)
            .and_then(|value| pyo3::IntoPyObjectExt::into_bound_py_any(value, py))?;
        let projector: Bound<'_, PyAny> = current_native_progress_projector(py)?;
        if !projector.is_none() {
            let options: Bound<'_, PyDict> = PyDict::new(py);
            options.set_item("ordinals", source_order.clone())?;
            options.set_item("total", total_count)?;
            projector.call_method("configure_resources", (), Some(&options))?;
        }
        Ok(Self {
            is_tty: stream.call_method0("isatty")?.is_truthy()?,
            stream: stream.unbind(),
            write_lock: native_field(&lines, "lock")?.unbind(),
            lines: lines.unbind(),
            style: CliStyle {
                use_color: arguments.value("use_color")?.is_truthy()?,
            },
            source_order,
            total: total_count,
            projector: (!projector.is_none()).then(|| projector.unbind()),
            state: Mutex::new(ReporterState::default()),
        })
    }

    fn on_start(slf: &Bound<'_, Self>, source: &Bound<'_, PyAny>) -> PyResult<()> {
        let py: Python<'_> = slf.py();
        let reporter: &Self = slf.get();
        if reporter.projector.is_some() || !reporter.is_tty {
            return Ok(());
        }
        let loader_node: bool = native_field(source, "meta")?
            .call_method1("get", (LOADER_NODE_KEY,))?
            .is(PyBool::new(py, true).to_owned());
        {
            let mut state = reporter.state();
            state.active = Some(ActiveSource {
                name: native_field(source, "name")?.extract()?,
                resource_kind: if loader_node { "loader" } else { "source" }.to_owned(),
            });
            state.message = String::new();
        }
        reporter.set_cursor(py, true)?;
        reporter.locked(py, || {
            reporter.state().line_active = true;
            let options: Bound<'_, PyDict> = PyDict::new(py);
            options.set_item("stream", reporter.stream.bind(py))?;
            options.set_item("owner", slf)?;
            reporter
                .lines
                .bind(py)
                .call_method("claim", (), Some(&options))?;
            reporter.write_spinner_line(py)
        })?;
        Self::start_spinner(slf)
    }

    fn on_progress(
        &self,
        py: Python<'_>,
        source: &Bound<'_, PyAny>,
        message: String,
    ) -> PyResult<()> {
        if !self.is_tty {
            return Ok(());
        }
        let name: String = native_field(source, "name")?.extract()?;
        {
            let mut state = self.state();
            match &state.active {
                Some(active) if active.name == name => state.message = message,
                _ => return Ok(()),
            }
        }
        self.write_spinner_line(py)
    }

    fn on_complete(slf: &Bound<'_, Self>, result: &Bound<'_, PyAny>) -> PyResult<()> {
        let py: Python<'_> = slf.py();
        let reporter: &Self = slf.get();
        let text: LoadResultText = load_result_text(result)?;
        let mut duration_ms: Option<f64> = text.duration_ms;
        if let Some(projector) = &reporter.projector {
            let options: Bound<'_, PyDict> = PyDict::new(py);
            options.set_item("resource_name", &text.source_name)?;
            options.set_item("resource_id", format!("source:{}", text.source_name))?;
            let canonical: Bound<'_, PyAny> =
                projector
                    .bind(py)
                    .call_method("consume_resource_terminal", (), Some(&options))?;
            if canonical.is_none() {
                return Ok(());
            }
            duration_ms = Some(canonical.extract()?);
        }
        reporter.stop_spinner(py)?;
        if reporter.is_tty {
            reporter.locked(py, || {
                reporter.state().line_active = false;
                let options: Bound<'_, PyDict> = PyDict::new(py);
                options.set_item("owner", slf)?;
                reporter
                    .lines
                    .bind(py)
                    .call_method("release", (), Some(&options))?;
                reporter.write(py, CLEAR_LINE)
            })?;
            reporter.set_cursor(py, false)?;
        }
        let ordinal: i64 = *reporter
            .source_order
            .get(&text.source_name)
            .ok_or_else(|| pyo3::exceptions::PyKeyError::new_err(text.source_name.clone()))?;
        reporter.write(
            py,
            &render_load_result(
                &text,
                (ordinal, reporter.total),
                duration_ms,
                reporter.style,
            ),
        )
    }

    /// Erase the live spinner row so a persistent line can take its place.
    fn clear_transient_line(&self, py: Python<'_>) -> PyResult<()> {
        self.locked(py, || self.write(py, CLEAR_LINE))
    }

    /// Draw the live spinner row again below persistent output.
    fn redraw_transient_line(&self, py: Python<'_>) -> PyResult<()> {
        self.write_spinner_line(py)
    }

    fn _spin_until_stopped(&self, py: Python<'_>, stop_event: &Bound<'_, PyAny>) -> PyResult<()> {
        while !stop_event
            .call_method1("wait", (SPINNER_TICK_SECONDS,))?
            .is_truthy()?
        {
            self.write_spinner_line(py)?;
        }
        Ok(())
    }
}

/// Forward load completions to the reporter and event output, and connection timing to its reporter.
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct LoadCallbackRelay {
    pub(crate) reporter: Py<LoadProgressReporter>,
    pub(crate) event_writer: Py<PyAny>,
    pub(crate) connection: Py<PyAny>,
}

#[pymethods]
impl LoadCallbackRelay {
    fn on_load_complete(&self, py: Python<'_>, result: &Bound<'_, PyAny>) -> PyResult<()> {
        LoadProgressReporter::on_complete(self.reporter.bind(py), result)?;
        let options: Bound<'_, PyDict> = PyDict::new(py);
        options.set_item("result", result)?;
        options.set_item("plan", py.None())?;
        options.set_item("command", PyString::new(py, "load"))?;
        self.event_writer
            .bind(py)
            .call_method("write_build_result", (), Some(&options))?;
        Ok(())
    }

    fn on_connection_complete(
        &self,
        py: Python<'_>,
        connection_count: i64,
        elapsed_seconds: f64,
    ) -> PyResult<()> {
        self.connection_call(
            py,
            "on_connection_complete",
            (connection_count, elapsed_seconds),
        )
    }

    fn on_connection_error(
        &self,
        py: Python<'_>,
        connection_count: i64,
        elapsed_seconds: f64,
    ) -> PyResult<()> {
        self.connection_call(
            py,
            "on_connection_error",
            (connection_count, elapsed_seconds),
        )
    }
}

impl LoadCallbackRelay {
    fn connection_call(
        &self,
        py: Python<'_>,
        method: &str,
        (connection_count, elapsed_seconds): (i64, f64),
    ) -> PyResult<()> {
        let options: Bound<'_, PyDict> = PyDict::new(py);
        options.set_item("connection_count", connection_count)?;
        options.set_item("elapsed_seconds", elapsed_seconds)?;
        self.connection
            .bind(py)
            .call_method(method, (), Some(&options))?;
        Ok(())
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<LoadProgressReporter>()?;
    Ok(())
}
