//! Aggregate live state and plan-ordered result rows of standalone concurrent audits.

use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard, PoisonError};

use pyo3::exceptions::PyKeyError;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::PyDict;
use pyo3::{pyclass, pymethods};
use sqlbuild_core::build_progress::models::AuditResultText;
use sqlbuild_core::cli_text::models::CliStyle;
use sqlbuild_core::nested_progress::main::audit_group_name::audit_group_name;
use sqlbuild_core::nested_progress::main::audit_resource_id::audit_resource_id;
use sqlbuild_core::nested_progress::main::render_audit_aggregate::render_audit_aggregate;
use sqlbuild_core::nested_progress::main::render_audit_row::render_audit_row;
use sqlbuild_core::nested_progress::main::render_group_header::render_group_header;
use sqlbuild_core::nested_progress::models::AuditAggregate;

use crate::bindings::_helpers::build_progress::result_text::{audit_text, optional, text};
use sqlbuild_python_base::boundary::main::within::within;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::build_progress::constants::CLEAR_LINE;
use sqlbuild_python_base::build_progress::constants::HIDE_CURSOR;
use sqlbuild_python_base::build_progress::constants::SHOW_CURSOR;
use sqlbuild_python_base::build_progress::main::terminal::claim::claim;
use sqlbuild_python_base::build_progress::main::terminal::consume_terminal::consume_terminal;
use sqlbuild_python_base::build_progress::main::terminal::flush::flush;
use sqlbuild_python_base::build_progress::main::terminal::projector::projector;
use sqlbuild_python_base::build_progress::main::terminal::release::release;
use sqlbuild_python_base::build_progress::main::terminal::shared_lines::shared_lines;
use sqlbuild_python_base::build_progress::main::terminal::tty_flag::tty_flag;
use sqlbuild_python_base::build_progress::main::terminal::write::write;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// The identity an audit plan entry or result reports to the progress projector.
fn resource_id(value: &Bound<'_, PyAny>, name_field: &str) -> PyResult<String> {
    let kind: Option<String> = match optional(value, "attached_target_kind")? {
        Some(kind) => Some(kind),
        None => Some(text(value, "attachment_kind")?),
    };
    Ok(audit_resource_id(
        &text(value, name_field)?,
        kind.as_deref().unwrap_or(""),
        (
            optional(value, "attached_target_name")?.as_deref(),
            optional(value, "attached_column_name")?.as_deref(),
        ),
    ))
}

/// Mutable reporter state; only held briefly and never across Python calls.
#[derive(Default)]
struct ReporterState {
    completed_by_index: HashMap<usize, Option<Py<PyAny>>>,
    next_index: usize,
    running: usize,
    completed: usize,
    current_group: Option<String>,
    closed: bool,
    cursor_hidden: bool,
    on_result: Option<Py<PyAny>>,
}

/// Render aggregate live state and flush completed audit rows in plan order.
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct AuditProgressReporter {
    stream: Py<PyAny>,
    lines: Py<PyAny>,
    lock: Py<PyAny>,
    style: CliStyle,
    is_tty: bool,
    projector: Option<Py<PyAny>>,
    indexes: HashMap<String, usize>,
    total: usize,
    worker_limit: i64,
    state: Mutex<ReporterState>,
}

impl AuditProgressReporter {
    fn state(&self) -> MutexGuard<'_, ReporterState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn index_of(&self, identity: String) -> PyResult<usize> {
        self.indexes
            .get(&identity)
            .copied()
            .ok_or_else(|| PyKeyError::new_err(identity))
    }

    fn render_aggregate(slf: &Bound<'_, Self>) -> PyResult<()> {
        let py: Python<'_> = slf.py();
        let reporter: &Self = slf.get();
        let (closed, cursor_hidden, running, completed) = {
            let state = reporter.state();
            (
                state.closed,
                state.cursor_hidden,
                state.running,
                state.completed,
            )
        };
        if !reporter.is_tty || closed {
            return Ok(());
        }
        let stream: &Bound<'_, PyAny> = reporter.stream.bind(py);
        claim(reporter.lines.bind(py), stream, slf.as_any())?;
        if !cursor_hidden {
            write(stream, HIDE_CURSOR)?;
            reporter.state().cursor_hidden = true;
        }
        write(
            stream,
            &render_audit_aggregate(AuditAggregate {
                running,
                completed,
                total: reporter.total,
                workers: reporter.worker_limit,
            }),
        )?;
        flush(stream)
    }

    fn flush_ready(&self, py: Python<'_>) -> PyResult<()> {
        loop {
            let ready: Option<Py<PyAny>> = {
                let mut state = self.state();
                let index: usize = state.next_index;
                let Some(result) = state.completed_by_index.remove(&index) else {
                    return Ok(());
                };
                state.next_index += 1;
                result
            };
            if let Some(result) = ready {
                self.write_result(py, result.bind(py))?;
            }
        }
    }

    fn write_result(&self, py: Python<'_>, result: &Bound<'_, PyAny>) -> PyResult<()> {
        let stream: &Bound<'_, PyAny> = self.stream.bind(py);
        if self.is_tty {
            write(stream, CLEAR_LINE)?;
        }
        let audit: AuditResultText = audit_text(result)?;
        let group: String = audit_group_name(&audit);
        let previous: Option<String> = self.state().current_group.clone();
        if previous.as_ref() != Some(&group) {
            write(
                stream,
                &render_group_header(&group, previous.is_none(), self.style),
            )?;
            self.state().current_group = Some(group);
        }
        let duration_ms: Option<f64> = match &self.projector {
            Some(projector) => consume_terminal(
                projector.bind(py),
                &audit.audit_name,
                Some(&resource_id(result, "audit_name")?),
            )?,
            None => None,
        };
        write(stream, &render_audit_row(&audit, duration_ms, self.style))?;
        flush(stream)?;
        let callback: Option<Py<PyAny>> = self
            .state()
            .on_result
            .as_ref()
            .map(|callback| callback.clone_ref(py));
        if let Some(callback) = callback {
            callback.bind(py).call1((result,))?;
        }
        Ok(())
    }

    fn locked<T>(&self, py: Python<'_>, body: impl FnOnce() -> PyResult<T>) -> PyResult<T> {
        within(self.lock.bind(py), |_| body())
    }
}

#[pymethods]
impl AuditProgressReporter {
    #[new]
    #[pyo3(signature = (**options))]
    pub(crate) fn new(py: Python<'_>, options: Option<Bound<'_, PyDict>>) -> PyResult<Self> {
        let arguments = KeywordArguments::read(
            py,
            "AuditProgressReporter",
            options,
            (&["entries", "worker_limit", "stream", "use_color"], &[]),
        )?;
        let stream: Bound<'_, PyAny> = arguments.value("stream")?;
        let lines: Bound<'_, PyAny> = shared_lines(py)?;
        let mut indexes: HashMap<String, usize> = HashMap::new();
        let mut total: usize = 0;
        for entry in arguments.value("entries")?.try_iter()? {
            indexes.insert(resource_id(&entry?, "name")?, total);
            total += 1;
        }
        Ok(Self {
            is_tty: tty_flag(&stream)?,
            stream: stream.unbind(),
            lock: native_field(&lines, "lock")?.unbind(),
            lines: lines.unbind(),
            style: CliStyle {
                use_color: arguments.value("use_color")?.is_truthy()?,
            },
            projector: projector(py)?,
            indexes,
            total,
            worker_limit: arguments.value("worker_limit")?.extract()?,
            state: Mutex::new(ReporterState::default()),
        })
    }

    /// Install plan-ordered result enrichment before dispatch starts.
    fn set_result_callback(&self, callback: Py<PyAny>) {
        self.state().on_result = Some(callback);
    }

    /// Record a worker start without mutating one-item spinner state.
    fn on_item_start(slf: &Bound<'_, Self>, entry: &Bound<'_, PyAny>) -> PyResult<()> {
        let _ = entry;
        slf.get().locked(slf.py(), || {
            slf.get().state().running += 1;
            Self::render_aggregate(slf)
        })
    }

    /// Flush every newly contiguous plan-order result row.
    fn on_item_complete(slf: &Bound<'_, Self>, result: &Bound<'_, PyAny>) -> PyResult<()> {
        let py: Python<'_> = slf.py();
        let reporter: &Self = slf.get();
        reporter.locked(py, || {
            let index: usize = reporter.index_of(resource_id(result, "audit_name")?)?;
            reporter
                .state()
                .completed_by_index
                .insert(index, Some(result.clone().unbind()));
            reporter.flush_ready(py)?;
            Self::render_aggregate(slf)
        })
    }

    /// Update aggregate state immediately without projecting a result row.
    fn on_item_physical_complete(slf: &Bound<'_, Self>, result: &Bound<'_, PyAny>) -> PyResult<()> {
        let _ = result;
        slf.get().locked(slf.py(), || {
            {
                let mut state = slf.get().state();
                state.running = state.running.saturating_sub(1);
                state.completed += 1;
            }
            Self::render_aggregate(slf)
        })
    }

    /// Advance aggregate and ordering state for a fatal audit exception.
    fn on_item_error(slf: &Bound<'_, Self>, entry: &Bound<'_, PyAny>) -> PyResult<()> {
        let py: Python<'_> = slf.py();
        let reporter: &Self = slf.get();
        reporter.locked(py, || {
            let index: usize = reporter.index_of(resource_id(entry, "name")?)?;
            {
                let mut state = reporter.state();
                state.running = state.running.saturating_sub(1);
                state.completed += 1;
                state.completed_by_index.insert(index, None);
            }
            reporter.flush_ready(py)?;
            Self::render_aggregate(slf)
        })
    }

    /// Restore the terminal after the final aggregate update.
    fn close(slf: &Bound<'_, Self>) -> PyResult<()> {
        let py: Python<'_> = slf.py();
        let reporter: &Self = slf.get();
        reporter.locked(py, || {
            reporter.state().closed = true;
            release(reporter.lines.bind(py), slf.as_any())?;
            if !reporter.is_tty {
                return Ok(());
            }
            let stream: &Bound<'_, PyAny> = reporter.stream.bind(py);
            write(stream, CLEAR_LINE)?;
            let hidden: bool = std::mem::take(&mut reporter.state().cursor_hidden);
            if hidden {
                write(stream, SHOW_CURSOR)?;
            }
            flush(stream)
        })
    }

    /// Erase the aggregate row so a persistent line can take its place.
    fn clear_transient_line(&self, py: Python<'_>) -> PyResult<()> {
        self.locked(py, || {
            let stream: &Bound<'_, PyAny> = self.stream.bind(py);
            write(stream, CLEAR_LINE)?;
            flush(stream)
        })
    }

    /// Draw the aggregate row again below persistent output.
    fn redraw_transient_line(slf: &Bound<'_, Self>) -> PyResult<()> {
        slf.get().locked(slf.py(), || Self::render_aggregate(slf))
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<AuditProgressReporter>()
}
