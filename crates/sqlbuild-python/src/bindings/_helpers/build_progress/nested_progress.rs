//! One live spinner row for all running items and grouped completed rows of `sqb test`.

use std::sync::{Mutex, MutexGuard, PoisonError};

use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::PyDict;
use pyo3::{pyclass, pymethods};
use sqlbuild_core::build_progress::models::ErrorText;
use sqlbuild_core::cli_text::models::CliStyle;
use sqlbuild_core::nested_progress::main::render_group_header::render_group_header;
use sqlbuild_core::nested_progress::main::render_nested_item::render_nested_item;
use sqlbuild_core::nested_progress::main::render_nested_spinner_line::render_nested_spinner_line;
use sqlbuild_core::nested_progress::models::{NestedChildRow, NestedItem, NestedLayout};

use crate::bindings::_helpers::build_progress::result_text::text;
use sqlbuild_python_base::boundary::main::within::within;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::build_progress::constants::CLEAR_LINE;
use sqlbuild_python_base::build_progress::constants::HIDE_CURSOR;
use sqlbuild_python_base::build_progress::constants::SHOW_CURSOR;
use sqlbuild_python_base::build_progress::constants::SPINNER_FRAMES;
use sqlbuild_python_base::build_progress::constants::SPINNER_TICK_SECONDS;
use sqlbuild_python_base::build_progress::main::terminal::claim::claim;
use sqlbuild_python_base::build_progress::main::terminal::consume_terminal::consume_terminal;
use sqlbuild_python_base::build_progress::main::terminal::flush::flush;
use sqlbuild_python_base::build_progress::main::terminal::join_spinner::join_spinner;
use sqlbuild_python_base::build_progress::main::terminal::projector::projector;
use sqlbuild_python_base::build_progress::main::terminal::release::release;
use sqlbuild_python_base::build_progress::main::terminal::shared_lines::shared_lines;
use sqlbuild_python_base::build_progress::main::terminal::start_spinner::start_spinner;
use sqlbuild_python_base::build_progress::main::terminal::terminal_columns::terminal_columns;
use sqlbuild_python_base::build_progress::main::terminal::tty_flag::tty_flag;
use sqlbuild_python_base::build_progress::main::terminal::write::write;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

const MIN_NAME_WIDTH: usize = 50;
const ITEM_OPTIONS: (&[&str], &[&str]) = (
    &["group_name", "item_name"],
    &[
        "status_text",
        "detail",
        "error_code",
        "error_help",
        "error_message",
        "child_rows",
        "canonical_resource_name",
        "canonical_resource_id",
    ],
);

/// Mutable reporter state; only held briefly and never across Python calls.
#[derive(Default)]
struct ReporterState {
    printed_group: Option<String>,
    running: Vec<(String, String)>,
    frame: usize,
    line_active: bool,
    cursor_hidden: bool,
    stop_event: Option<Py<PyAny>>,
    thread: Option<Py<PyAny>>,
}

/// Render one live spinner row for all running items and grouped completed rows.
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct NestedCommandProgressCallbacks {
    stream: Py<PyAny>,
    lines: Py<PyAny>,
    lock: Py<PyAny>,
    label: String,
    name_width: usize,
    style: CliStyle,
    is_tty: bool,
    sequential: bool,
    projector: Option<Py<PyAny>>,
    state: Mutex<ReporterState>,
}

fn optional_argument(arguments: &KeywordArguments<'_>, name: &str) -> PyResult<Option<String>> {
    if !arguments.passed(name)? {
        return Ok(None);
    }
    let value: Bound<'_, PyAny> = arguments.value(name)?;
    if value.is_none() {
        return Ok(None);
    }
    value.extract().map(Some)
}

fn child_rows(arguments: &KeywordArguments<'_>) -> PyResult<Vec<NestedChildRow>> {
    if !arguments.passed("child_rows")? {
        return Ok(Vec::new());
    }
    let mut rows: Vec<NestedChildRow> = Vec::new();
    for row in arguments.value("child_rows")?.try_iter()? {
        let row: Bound<'_, PyAny> = row?;
        rows.push(NestedChildRow {
            label: text(&row, "label")?,
            name: text(&row, "name")?,
            status_text: text(&row, "status_text")?,
            detail: text(&row, "detail")?,
        });
    }
    Ok(rows)
}

impl NestedCommandProgressCallbacks {
    fn state(&self) -> MutexGuard<'_, ReporterState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn locked<T>(&self, py: Python<'_>, body: impl FnOnce() -> PyResult<T>) -> PyResult<T> {
        within(self.lock.bind(py), |_| body())
    }

    fn layout(&self, py: Python<'_>) -> PyResult<NestedLayout> {
        Ok(NestedLayout {
            label: self.label.clone(),
            name_width: self.name_width,
            columns: terminal_columns(py)?,
        })
    }

    fn draw_spinner_line(&self, py: Python<'_>) -> PyResult<()> {
        let (running, frame) = {
            let mut state = self.state();
            let frame: usize = state.frame;
            state.frame = (frame + 1) % SPINNER_FRAMES.len();
            let running: Vec<String> = state.running.iter().map(|(item, _)| item.clone()).collect();
            (running, frame)
        };
        let stream: &Bound<'_, PyAny> = self.stream.bind(py);
        write(
            stream,
            &render_nested_spinner_line(
                &running,
                SPINNER_FRAMES[frame],
                &self.layout(py)?,
                self.style,
            ),
        )?;
        flush(stream)
    }

    fn clear_live_row(&self, py: Python<'_>) -> PyResult<()> {
        if self.is_tty && self.state().line_active {
            write(self.stream.bind(py), CLEAR_LINE)?;
        }
        Ok(())
    }

    fn write_group_header(&self, py: Python<'_>, group: &str) -> PyResult<()> {
        let previous: Option<String> = self.state().printed_group.clone();
        if previous.as_deref() == Some(group) {
            return Ok(());
        }
        self.state().printed_group = Some(group.to_owned());
        let stream: &Bound<'_, PyAny> = self.stream.bind(py);
        write(
            stream,
            &render_group_header(group, previous.is_none(), self.style),
        )?;
        flush(stream)
    }

    fn set_cursor(&self, py: Python<'_>, hidden: bool) -> PyResult<()> {
        if self.state().cursor_hidden == hidden {
            return Ok(());
        }
        let stream: &Bound<'_, PyAny> = self.stream.bind(py);
        write(stream, if hidden { HIDE_CURSOR } else { SHOW_CURSOR })?;
        flush(stream)?;
        self.state().cursor_hidden = hidden;
        Ok(())
    }

    /// Redraw the live row for remaining items, or retire it; return a thread to join.
    fn refresh_live_row(slf: &Bound<'_, Self>) -> PyResult<Option<Py<PyAny>>> {
        let py: Python<'_> = slf.py();
        let reporter: &Self = slf.get();
        if !reporter.is_tty {
            return Ok(None);
        }
        if !reporter.state().running.is_empty() {
            reporter.draw_spinner_line(py)?;
            return Ok(None);
        }
        reporter.state().line_active = false;
        release(reporter.lines.bind(py), slf.as_any())?;
        reporter.set_cursor(py, false)?;
        let (event, thread) = {
            let mut state = reporter.state();
            (state.stop_event.take(), state.thread.take())
        };
        if let Some(event) = event {
            event.bind(py).call_method0("set")?;
        }
        Ok(thread)
    }
}

#[pymethods]
impl NestedCommandProgressCallbacks {
    #[new]
    #[pyo3(signature = (**options))]
    pub(crate) fn new(py: Python<'_>, options: Option<Bound<'_, PyDict>>) -> PyResult<Self> {
        let arguments = KeywordArguments::read(
            py,
            "NestedCommandProgressCallbacks",
            options,
            (
                &["total", "label", "stream", "use_color"],
                &["name_width", "concurrency"],
            ),
        )?;
        let stream: Bound<'_, PyAny> = arguments.value("stream")?;
        let lines: Bound<'_, PyAny> = shared_lines(py)?;
        let name_width: usize = if arguments.passed("name_width")? {
            arguments.value("name_width")?.extract()?
        } else {
            MIN_NAME_WIDTH
        };
        let concurrency: i64 = if arguments.passed("concurrency")? {
            arguments.value("concurrency")?.extract()?
        } else {
            1
        };
        Ok(Self {
            is_tty: tty_flag(&stream)?,
            stream: stream.unbind(),
            lock: native_field(&lines, "lock")?.unbind(),
            lines: lines.unbind(),
            label: arguments.value("label")?.extract()?,
            name_width: name_width.max(MIN_NAME_WIDTH),
            style: CliStyle {
                use_color: arguments.value("use_color")?.is_truthy()?,
            },
            sequential: concurrency <= 1,
            projector: projector(py)?,
            state: Mutex::new(ReporterState::default()),
        })
    }

    #[pyo3(signature = (**options))]
    fn on_item_start(slf: &Bound<'_, Self>, options: Option<Bound<'_, PyDict>>) -> PyResult<()> {
        let py: Python<'_> = slf.py();
        let reporter: &Self = slf.get();
        let arguments = KeywordArguments::read(
            py,
            "on_item_start",
            options,
            (&["group_name", "item_name"], &["canonical_resource_name"]),
        )?;
        let group: String = arguments.value("group_name")?.extract()?;
        let item: String = arguments.value("item_name")?.extract()?;
        if let Some(projector) = &reporter.projector {
            let canonical: Option<String> =
                optional_argument(&arguments, "canonical_resource_name")?;
            let options: Bound<'_, PyDict> = PyDict::new(py);
            options.set_item(
                "resource_name",
                canonical
                    .filter(|name| !name.is_empty())
                    .unwrap_or(item.clone()),
            )?;
            projector
                .bind(py)
                .call_method("expect_resource_enrichment", (), Some(&options))?;
        }
        reporter.locked(py, || {
            if reporter.sequential && reporter.state().running.is_empty() {
                reporter.clear_live_row(py)?;
                reporter.write_group_header(py, &group)?;
            }
            {
                let mut state = reporter.state();
                match state.running.iter_mut().find(|(name, _)| *name == item) {
                    Some(entry) => entry.1 = group,
                    None => state.running.push((item, group)),
                }
            }
            if !reporter.is_tty {
                return Ok(());
            }
            reporter.set_cursor(py, true)?;
            reporter.state().line_active = true;
            claim(
                reporter.lines.bind(py),
                reporter.stream.bind(py),
                slf.as_any(),
            )?;
            reporter.draw_spinner_line(py)?;
            if reporter.state().thread.is_none() {
                let (event, thread) = start_spinner(slf.as_any())?;
                let mut state = reporter.state();
                state.stop_event = Some(event.unbind());
                state.thread = Some(thread.unbind());
            }
            Ok(())
        })
    }

    #[pyo3(signature = (**options))]
    fn on_item_complete(slf: &Bound<'_, Self>, options: Option<Bound<'_, PyDict>>) -> PyResult<()> {
        let py: Python<'_> = slf.py();
        let reporter: &Self = slf.get();
        let arguments = KeywordArguments::read(py, "on_item_complete", options, ITEM_OPTIONS)?;
        let group: String = arguments.value("group_name")?.extract()?;
        let item_name: String = arguments.value("item_name")?.extract()?;
        let mut detail: String = optional_argument(&arguments, "detail")?.unwrap_or_default();
        let duration_ms: Option<f64> = match &reporter.projector {
            Some(projector) => {
                let canonical: Option<String> =
                    optional_argument(&arguments, "canonical_resource_name")?;
                consume_terminal(
                    projector.bind(py),
                    &canonical
                        .filter(|name| !name.is_empty())
                        .unwrap_or(item_name.clone()),
                    optional_argument(&arguments, "canonical_resource_id")?.as_deref(),
                )?
            }
            None => None,
        };
        let stopped: Option<Py<PyAny>> = reporter.locked(py, || {
            let item_group: String = {
                let mut state = reporter.state();
                match state
                    .running
                    .iter()
                    .position(|(name, _)| *name == item_name)
                {
                    Some(index) => state.running.remove(index).1,
                    None => group,
                }
            };
            if reporter.projector.is_none() || duration_ms.is_some() {
                if let Some(duration_ms) = duration_ms {
                    detail = format!("{detail}  {:.2}s", duration_ms / 1000.0);
                }
                let item: NestedItem = NestedItem {
                    item_name,
                    status_text: optional_argument(&arguments, "status_text")?.unwrap_or_default(),
                    detail,
                    error: ErrorText {
                        code: optional_argument(&arguments, "error_code")?,
                        message: optional_argument(&arguments, "error_message")?,
                        help: optional_argument(&arguments, "error_help")?,
                    },
                    child_rows: child_rows(&arguments)?,
                };
                reporter.clear_live_row(py)?;
                reporter.write_group_header(py, &item_group)?;
                write(
                    reporter.stream.bind(py),
                    &render_nested_item(&item, &reporter.layout(py)?, reporter.style),
                )?;
            }
            let stopped: Option<Py<PyAny>> = Self::refresh_live_row(slf)?;
            flush(reporter.stream.bind(py))?;
            Ok(stopped)
        })?;
        if let Some(thread) = stopped {
            join_spinner(thread.bind(py))?;
        }
        Ok(())
    }

    /// Erase the live spinner row so a persistent line can take its place.
    fn clear_transient_line(&self, py: Python<'_>) -> PyResult<()> {
        self.locked(py, || {
            let stream: &Bound<'_, PyAny> = self.stream.bind(py);
            write(stream, CLEAR_LINE)?;
            flush(stream)
        })
    }

    /// Draw the live spinner row again below persistent output.
    fn redraw_transient_line(&self, py: Python<'_>) -> PyResult<()> {
        self.locked(py, || {
            if self.state().line_active {
                self.draw_spinner_line(py)?;
            }
            Ok(())
        })
    }

    fn _spin_until_stopped(&self, py: Python<'_>, stop_event: &Bound<'_, PyAny>) -> PyResult<()> {
        while !stop_event
            .call_method1("wait", (SPINNER_TICK_SECONDS,))?
            .is_truthy()?
        {
            self.locked(py, || {
                if !stop_event.call_method0("is_set")?.is_truthy()? && self.state().line_active {
                    self.draw_spinner_line(py)?;
                }
                Ok(())
            })?;
        }
        Ok(())
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<NestedCommandProgressCallbacks>()
}
