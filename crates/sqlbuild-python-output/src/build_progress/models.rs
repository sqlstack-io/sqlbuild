//! Public records of build progress.

use crate::build_progress::_helpers::audit_progress::resource_id;
use crate::build_progress::_helpers::build_callbacks::CallbackState;
use crate::build_progress::_helpers::build_callbacks::PASS_OUTCOME;
use crate::build_progress::_helpers::build_callbacks::QUERY_DIAGNOSTIC_LIMIT;
use crate::build_progress::_helpers::build_callbacks::RUNTIME_LOGGER;
use crate::build_progress::_helpers::build_callbacks::ResultKind;
use crate::build_progress::_helpers::build_callbacks::SCHEDULER_DIAGNOSTIC_LIMIT;
use crate::build_progress::_helpers::build_callbacks::resource_identity;
use crate::build_progress::_helpers::build_callbacks::result_kind;
use crate::build_progress::_helpers::build_callbacks::scheduler_state;
use crate::build_progress::_helpers::build_callbacks::statement;
use crate::build_progress::_helpers::heartbeat;
use crate::build_progress::_helpers::nested_progress::ITEM_OPTIONS;
use crate::build_progress::_helpers::nested_progress::MIN_NAME_WIDTH;
use crate::build_progress::_helpers::nested_progress::child_rows;
use crate::build_progress::_helpers::nested_progress::optional_argument;
use crate::build_progress::_helpers::plan_facts::BuildPlanFacts;
use crate::build_progress::_helpers::plan_facts::build_plan_facts;
use crate::build_progress::_helpers::progress_reporters::CONNECTION_OPTIONS;
use crate::build_progress::_helpers::progress_reporters::PROJECT_COMPILE_OPERATION;
use crate::build_progress::_helpers::result_text::audit_text;
use crate::build_progress::_helpers::result_text::function_text;
use crate::build_progress::_helpers::result_text::load_text;
use crate::build_progress::_helpers::result_text::model_text;
use crate::build_progress::_helpers::result_text::seed_text;
use crate::build_progress::_helpers::result_text::test_text;
use crate::build_progress::_helpers::result_text::text;
use crate::result_output::models::ExecutionEventWriter;
use pyo3::exceptions::PyKeyError;
use pyo3::prelude::Bound;
use pyo3::prelude::Py;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyAnyMethods;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::pyclass;
use pyo3::pymethods;
use pyo3::types::PyDict;
use pyo3::types::PyDictMethods;
use sqlbuild_core::build_progress::main::batch_progress_message::batch_progress_message;
use sqlbuild_core::build_progress::main::query_diagnostic_message::query_diagnostic_message;
use sqlbuild_core::build_progress::main::render_build_spinner_line::render_build_spinner_line;
use sqlbuild_core::build_progress::main::render_failed_test_result::render_failed_test_result;
use sqlbuild_core::build_progress::main::render_model_result::render_model_result;
use sqlbuild_core::build_progress::main::render_resource_result::render_resource_result;
use sqlbuild_core::build_progress::main::runtime_diagnostic_summary::runtime_diagnostic_summary;
use sqlbuild_core::build_progress::main::scheduler_diagnostic_message::scheduler_diagnostic_message;
use sqlbuild_core::build_progress::models::AuditResultText;
use sqlbuild_core::build_progress::models::BatchProgressText;
use sqlbuild_core::build_progress::models::ErrorText;
use sqlbuild_core::build_progress::models::ModelRowFacts;
use sqlbuild_core::build_progress::models::ProgressLayout;
use sqlbuild_core::build_progress::models::ResourceResult;
use sqlbuild_core::build_progress::models::SchedulerStateText;
use sqlbuild_core::build_progress::models::SpinnerRow;
use sqlbuild_core::build_progress::models::StatementText;
use sqlbuild_core::build_progress::models::TestResultText;
use sqlbuild_core::cli_text::models::CliStyle;
use sqlbuild_core::nested_progress::main::audit_group_name::audit_group_name;
use sqlbuild_core::nested_progress::main::render_audit_aggregate::render_audit_aggregate;
use sqlbuild_core::nested_progress::main::render_audit_row::render_audit_row;
use sqlbuild_core::nested_progress::main::render_group_header::render_group_header;
use sqlbuild_core::nested_progress::main::render_nested_item::render_nested_item;
use sqlbuild_core::nested_progress::main::render_nested_spinner_line::render_nested_spinner_line;
use sqlbuild_core::nested_progress::models::AuditAggregate;
use sqlbuild_core::nested_progress::models::NestedItem;
use sqlbuild_core::nested_progress::models::NestedLayout;
use sqlbuild_core::progress::main::connection_phase_line::connection_phase_line;
use sqlbuild_core::progress::main::connection_start_message::connection_start_message;
use sqlbuild_core::progress::main::is_planning_completion_message::is_planning_completion_message;
use sqlbuild_core::progress::main::is_planning_failure_message::is_planning_failure_message;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::boundary::main::within::within;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::build_progress::constants::CLEAR_LINE;
use sqlbuild_python_base::build_progress::constants::HIDE_CURSOR;
use sqlbuild_python_base::build_progress::constants::SHOW_CURSOR;
use sqlbuild_python_base::build_progress::constants::SPINNER_FRAMES;
use sqlbuild_python_base::build_progress::constants::SPINNER_TICK_SECONDS;
use sqlbuild_python_base::build_progress::main::progress::current_projector::current_projector;
use sqlbuild_python_base::build_progress::main::progress::projector_operation_active::projector_operation_active;
use sqlbuild_python_base::build_progress::main::progress::write_persistent::write_persistent;
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
use sqlbuild_python_base::build_progress::main::transient_status::status_reporter::status_reporter;
use sqlbuild_python_base::build_progress::models::NativeProgressProjector;
use sqlbuild_python_base::invocation_phases::constants::CONNECTED_PHASE;
use sqlbuild_python_base::invocation_phases::models::InvocationPhase;
use sqlbuild_python_base::native_host::main::publish_invocation_phase::publish_invocation_phase;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::replaced_keywords::replaced_keywords;
use sqlbuild_python_project::project_snapshot::models::WorkerBuild;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::MutexGuard;
use std::sync::PoisonError;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Instant;

/// Render aggregate live state and flush completed audit rows in plan order.
#[pyclass(module = "sqlbuild._native", frozen)]
#[derive(Debug)]
pub struct AuditProgressReporter {
    pub(crate) stream: Py<PyAny>,
    pub(crate) lines: Py<PyAny>,
    pub(crate) lock: Py<PyAny>,
    pub(crate) style: CliStyle,
    pub(crate) is_tty: bool,
    pub(crate) projector: Option<Py<PyAny>>,
    pub(crate) indexes: HashMap<String, usize>,
    pub(crate) total: usize,
    pub(crate) worker_limit: i64,
    pub(crate) state: Mutex<crate::build_progress::_helpers::audit_progress::ReporterState>,
}

impl AuditProgressReporter {
    pub(crate) fn state(
        &self,
    ) -> MutexGuard<'_, crate::build_progress::_helpers::audit_progress::ReporterState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub(crate) fn index_of(&self, identity: String) -> PyResult<usize> {
        self.indexes
            .get(&identity)
            .copied()
            .ok_or_else(|| PyKeyError::new_err(identity))
    }

    pub(crate) fn render_aggregate(slf: &Bound<'_, Self>) -> PyResult<()> {
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

    pub(crate) fn flush_ready(&self, py: Python<'_>) -> PyResult<()> {
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

    pub(crate) fn write_result(&self, py: Python<'_>, result: &Bound<'_, PyAny>) -> PyResult<()> {
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

    pub(crate) fn locked<T>(
        &self,
        py: Python<'_>,
        body: impl FnOnce() -> PyResult<T>,
    ) -> PyResult<T> {
        within(self.lock.bind(py), |_| body())
    }
}

#[pymethods]
impl AuditProgressReporter {
    #[new]
    #[pyo3(signature = (**options))]
    pub fn new(py: Python<'_>, options: Option<Bound<'_, PyDict>>) -> PyResult<Self> {
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
            state: Mutex::new(
                crate::build_progress::_helpers::audit_progress::ReporterState::default(),
            ),
        })
    }

    /// Install plan-ordered result enrichment before dispatch starts.
    pub(crate) fn set_result_callback(&self, callback: Py<PyAny>) {
        self.state().on_result = Some(callback);
    }

    /// Record a worker start without mutating one-item spinner state.
    pub(crate) fn on_item_start(slf: &Bound<'_, Self>, entry: &Bound<'_, PyAny>) -> PyResult<()> {
        let _ = entry;
        slf.get().locked(slf.py(), || {
            slf.get().state().running += 1;
            Self::render_aggregate(slf)
        })
    }

    /// Flush every newly contiguous plan-order result row.
    pub(crate) fn on_item_complete(
        slf: &Bound<'_, Self>,
        result: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
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
    pub(crate) fn on_item_physical_complete(
        slf: &Bound<'_, Self>,
        result: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
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
    pub(crate) fn on_item_error(slf: &Bound<'_, Self>, entry: &Bound<'_, PyAny>) -> PyResult<()> {
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
    pub(crate) fn close(slf: &Bound<'_, Self>) -> PyResult<()> {
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
    pub(crate) fn clear_transient_line(&self, py: Python<'_>) -> PyResult<()> {
        self.locked(py, || {
            let stream: &Bound<'_, PyAny> = self.stream.bind(py);
            write(stream, CLEAR_LINE)?;
            flush(stream)
        })
    }

    /// Draw the aggregate row again below persistent output.
    pub(crate) fn redraw_transient_line(slf: &Bound<'_, Self>) -> PyResult<()> {
        slf.get().locked(slf.py(), || Self::render_aggregate(slf))
    }
}

/// Encapsulates live build progress output state and callbacks.
#[pyclass(module = "sqlbuild._native", frozen)]
#[derive(Debug)]
pub struct BuildProgressCallbacks {
    pub worker: Option<Arc<WorkerBuild>>,
    pub(crate) plan: Py<PyAny>,
    pub(crate) stream: Py<PyAny>,
    pub(crate) lines: Py<PyAny>,
    pub(crate) lock: Py<PyAny>,
    pub(crate) event_writer: Py<PyAny>,
    pub(crate) projector: Option<Py<PyAny>>,
    pub(crate) facts: BuildPlanFacts,
    pub(crate) style: CliStyle,
    pub(crate) verbose: bool,
    pub(crate) debug: bool,
    pub(crate) is_tty: bool,
    pub(crate) started: Instant,
    pub(crate) state: Mutex<CallbackState>,
}

impl BuildProgressCallbacks {
    pub(crate) fn state(&self) -> MutexGuard<'_, CallbackState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub(crate) fn locked<T>(
        &self,
        py: Python<'_>,
        body: impl FnOnce() -> PyResult<T>,
    ) -> PyResult<T> {
        within(self.lock.bind(py), |_| body())
    }

    pub(crate) fn layout(&self, py: Python<'_>) -> PyResult<ProgressLayout> {
        Ok(ProgressLayout {
            total: self.facts.total,
            name_width: self.facts.name_width,
            columns: terminal_columns(py)?,
            verbose: self.verbose,
        })
    }

    pub(crate) fn emit(&self, py: Python<'_>, text: &str) -> PyResult<()> {
        let stream: &Bound<'_, PyAny> = self.stream.bind(py);
        write(stream, text)?;
        flush(stream)
    }

    pub(crate) fn write_spinner_line(&self, py: Python<'_>) -> PyResult<()> {
        self.locked(py, || {
            let (row, frame) = {
                let mut state = self.state();
                if !state.line_active {
                    return Ok(());
                }
                let frame: usize = state.frame;
                state.frame = (frame + 1) % SPINNER_FRAMES.len();
                (
                    SpinnerRow {
                        position: state.counter + 1,
                        resource_kind: state.node_kind.clone(),
                        name: state.node_name.clone(),
                        message: state.sub_message.clone(),
                    },
                    frame,
                )
            };
            self.emit(
                py,
                &render_build_spinner_line(&row, frame, self.layout(py)?, self.style),
            )
        })
    }

    pub(crate) fn set_cursor(&self, py: Python<'_>, hidden: bool) -> PyResult<()> {
        if self.state().cursor_hidden == hidden {
            return Ok(());
        }
        self.locked(py, || {
            self.emit(py, if hidden { HIDE_CURSOR } else { SHOW_CURSOR })
        })?;
        self.state().cursor_hidden = hidden;
        Ok(())
    }

    pub(crate) fn stop_spinner(&self, py: Python<'_>) -> PyResult<()> {
        let (event, thread) = {
            let mut state = self.state();
            (state.stop_event.take(), state.thread.take())
        };
        if let Some(event) = event {
            event.bind(py).call_method0("set")?;
        }
        if let Some(thread) = thread {
            join_spinner(thread.bind(py))?;
        }
        Ok(())
    }

    /// Stop the spinner and erase its row before a persistent row is written.
    pub(crate) fn retire_live_row(slf: &Bound<'_, Self>) -> PyResult<()> {
        let py: Python<'_> = slf.py();
        let callbacks: &Self = slf.get();
        callbacks.stop_spinner(py)?;
        if !callbacks.is_tty {
            return Ok(());
        }
        callbacks.locked(py, || {
            callbacks.state().line_active = false;
            release(callbacks.lines.bind(py), slf.as_any())?;
            callbacks.emit(py, CLEAR_LINE)
        })?;
        callbacks.set_cursor(py, false)
    }

    pub(crate) fn write_runtime_diagnostic(&self, py: Python<'_>, message: &str) -> PyResult<()> {
        imported(py, "logging", "getLogger")?
            .call1((RUNTIME_LOGGER,))?
            .call_method1("debug", (message,))?;
        if self.debug {
            return Ok(());
        }
        self.write_persistent(py, message)
    }

    /// Write one permanent line through the shared line coordinator.
    pub(crate) fn write_persistent(&self, py: Python<'_>, message: &str) -> PyResult<()> {
        let options: Bound<'_, PyDict> = PyDict::new(py);
        options.set_item("stream", self.stream.bind(py))?;
        options.set_item("text", format!("{message}\n"))?;
        self.lines
            .bind(py)
            .call_method("write_persistent", (), Some(&options))?;
        Ok(())
    }

    /// Keep a line per completed microbatch when no live spinner row shows batch progress.
    pub(crate) fn write_batch_complete(
        &self,
        py: Python<'_>,
        progress: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        if self.is_tty {
            return Ok(());
        }
        let batch: BatchProgressText = BatchProgressText {
            model_name: text(progress, "model_name")?,
            batch_number: native_field(progress, "batch_number")?.extract()?,
            batch_count: native_field(progress, "batch_count")?.extract()?,
            window: text(progress, "window")?,
            rows_affected: native_field(progress, "rows_affected")?.extract()?,
            elapsed_seconds: native_field(progress, "elapsed_seconds")?.extract()?,
        };
        self.write_persistent(py, &batch_progress_message(&batch))
    }

    pub(crate) fn write_node_result(
        slf: &Bound<'_, Self>,
        result: &Bound<'_, PyAny>,
        kind: ResultKind,
    ) -> PyResult<()> {
        let py: Python<'_> = slf.py();
        let callbacks: &Self = slf.get();
        if kind == ResultKind::SqlTest {
            let test: TestResultText = test_text(result)?;
            if test.outcome != PASS_OUTCOME {
                Self::retire_live_row(slf)?;
                return callbacks.emit(
                    py,
                    &render_failed_test_result(&test, callbacks.layout(py)?, callbacks.style),
                );
            }
            let mut state = callbacks.state();
            for step in &test.steps {
                state
                    .tests_by_model
                    .entry(step.model_name.clone())
                    .or_default()
                    .push(test.clone());
            }
            return Ok(());
        }
        Self::retire_live_row(slf)?;
        let counter: usize = {
            let mut state = callbacks.state();
            state.counter += 1;
            state.counter
        };
        let layout: ProgressLayout = callbacks.layout(py)?;
        let style: CliStyle = callbacks.style;
        let text: String = match kind {
            ResultKind::Seed => render_resource_result(
                ResourceResult::Node(&seed_text(result)?),
                counter,
                layout,
                style,
            ),
            ResultKind::Function => render_resource_result(
                ResourceResult::Node(&function_text(result)?),
                counter,
                layout,
                style,
            ),
            ResultKind::Load => render_resource_result(
                ResourceResult::Load(&load_text(result)?),
                counter,
                layout,
                style,
            ),
            ResultKind::Model => {
                let model = model_text(result)?;
                let facts: &ModelRowFacts = callbacks
                    .facts
                    .models
                    .get(&model.name)
                    .unwrap_or(&callbacks.facts.unplanned_model);
                let tests: Vec<TestResultText> = callbacks
                    .state()
                    .tests_by_model
                    .get(&model.name)
                    .cloned()
                    .unwrap_or_default();
                render_model_result((&model, facts, &tests), counter, layout, style)
            }
            _ => return Ok(()),
        };
        callbacks.emit(py, &text)
    }

    pub(crate) fn write_event(&self, py: Python<'_>, result: &Bound<'_, PyAny>) -> PyResult<()> {
        let writer: &Bound<'_, PyAny> = self.event_writer.bind(py);
        let write_one = |item: &Bound<'_, PyAny>| -> PyResult<()> {
            let options: Bound<'_, PyDict> = PyDict::new(py);
            options.set_item("result", item)?;
            options.set_item("plan", self.plan.bind(py))?;
            writer.call_method("write_build_result", (), Some(&options))?;
            Ok(())
        };
        write_one(result)?;
        if result_kind(result)? == ResultKind::Model {
            for audit in native_field(result, "audit_results")?.try_iter()? {
                write_one(&audit?)?;
            }
        }
        Ok(())
    }
}

#[pymethods]
impl BuildProgressCallbacks {
    #[new]
    #[pyo3(signature = (**options))]
    pub fn new(py: Python<'_>, options: Option<Bound<'_, PyDict>>) -> PyResult<Self> {
        let arguments = KeywordArguments::read(
            py,
            "BuildProgressCallbacks",
            options,
            (
                &["plan", "use_color"],
                &["verbose", "debug", "event_output_path"],
            ),
        )?;
        let flag = |name: &str| -> PyResult<bool> {
            Ok(arguments.passed(name)? && arguments.value(name)?.is_truthy()?)
        };
        let plan: Bound<'_, PyAny> = arguments.value("plan")?;
        let debug: bool = flag("debug")?;
        let sys: Bound<'_, PyAny> = py.import("sys")?.into_any();
        let stdout: Bound<'_, PyAny> = native_field(&sys, "stdout")?;
        let stream: Bound<'_, PyAny> = if debug {
            native_field(&sys, "stderr")?
        } else {
            stdout.clone()
        };
        let path: Bound<'_, PyAny> = if arguments.passed("event_output_path")? {
            arguments.value("event_output_path")?
        } else {
            py.None().into_bound(py)
        };
        let event_writer: Bound<'_, PyAny> = Bound::new(
            py,
            ExecutionEventWriter::new(py, Some(path).filter(|value| !value.is_none()))?,
        )?
        .into_any();
        let facts: BuildPlanFacts = build_plan_facts(&plan)?;
        let projector: Option<Py<PyAny>> = projector(py)?;
        if let Some(projector) = &projector {
            let projector: &Bound<'_, PyAny> = projector.bind(py);
            let ordinals: Bound<'_, PyDict> = PyDict::new(py);
            for (name, ordinal) in &facts.ordinals {
                ordinals.set_item(name, ordinal)?;
            }
            let options: Bound<'_, PyDict> = PyDict::new(py);
            options.set_item("ordinals", ordinals)?;
            options.set_item("total", facts.total)?;
            projector.call_method("configure_resources", (), Some(&options))?;
            for name in &facts.test_names {
                let options: Bound<'_, PyDict> = PyDict::new(py);
                options.set_item("resource_name", name)?;
                projector.call_method("expect_resource_enrichment", (), Some(&options))?;
            }
        }
        let lines: Bound<'_, PyAny> = shared_lines(py)?;
        Ok(Self {
            worker: None,
            plan: plan.unbind(),
            is_tty: tty_flag(&stdout)? && !debug,
            stream: stream.unbind(),
            lock: native_field(&lines, "lock")?.unbind(),
            lines: lines.unbind(),
            event_writer: event_writer.unbind(),
            projector,
            facts,
            style: CliStyle {
                use_color: arguments.value("use_color")?.is_truthy()?,
            },
            verbose: flag("verbose")?,
            debug,
            started: Instant::now(),
            state: Mutex::new(CallbackState::default()),
        })
    }

    #[getter]
    pub fn elapsed(&self) -> f64 {
        self.started.elapsed().as_secs_f64()
    }

    #[pyo3(signature = (*, name, resource_kind))]
    pub(crate) fn on_node_start(
        slf: &Bound<'_, Self>,
        name: String,
        resource_kind: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let py: Python<'_> = slf.py();
        let callbacks: &Self = slf.get();
        if let Some(worker) = &callbacks.worker {
            worker.start(py, &name, resource_kind)?;
        }
        if !callbacks.is_tty {
            heartbeat::node_started(slf, &name);
        }
        if let Some(projector) = &callbacks.projector {
            let options: Bound<'_, PyDict> = PyDict::new(py);
            options.set_item("resource_name", name)?;
            projector
                .bind(py)
                .call_method("expect_resource_enrichment", (), Some(&options))?;
            return Ok(());
        }
        {
            let mut state = callbacks.state();
            state.node_name = name;
            state.node_kind = native_field(resource_kind, "value")?.extract()?;
            state.sub_message = String::new();
        }
        if !callbacks.is_tty {
            return Ok(());
        }
        callbacks.set_cursor(py, true)?;
        callbacks.locked(py, || {
            callbacks.state().line_active = true;
            claim(
                callbacks.lines.bind(py),
                callbacks.stream.bind(py),
                slf.as_any(),
            )?;
            callbacks.write_spinner_line(py)
        })?;
        callbacks.stop_spinner(py)?;
        let (event, thread) = start_spinner(slf.as_any())?;
        let mut state = callbacks.state();
        state.stop_event = Some(event.unbind());
        state.thread = Some(thread.unbind());
        Ok(())
    }

    pub(crate) fn on_sub_progress(&self, py: Python<'_>, message: String) -> PyResult<()> {
        self.state().sub_message = message;
        if self.is_tty {
            self.write_spinner_line(py)?;
        }
        Ok(())
    }

    /// Keep a line per completed microbatch when no live spinner row shows batch progress; a worker reports the rows so far.
    pub(crate) fn on_batch_complete(
        &self,
        py: Python<'_>,
        progress: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let name: String = text(progress, "model_name")?;
        let rows: Option<i64> = native_field(progress, "rows_affected")?.extract()?;
        if let Some(worker) = &self.worker {
            worker.batch_rows(py, &format!("model:{name}"), rows)?;
        }
        heartbeat::batch_rows(self, &name, rows);
        self.write_batch_complete(py, progress)
    }

    /// A worker reports the rows a running loader has written so far.
    pub(crate) fn on_rows_written(&self, py: Python<'_>, key: &str, rows: i64) -> PyResult<()> {
        heartbeat::rows_written(
            self,
            key.split_once(':').map_or(key, |(_, name)| name),
            rows,
        );
        match &self.worker {
            Some(worker) => worker.rows_written(py, key, rows),
            None => Ok(()),
        }
    }

    /// Report one deduplicated concurrent scheduler state transition.
    pub(crate) fn on_scheduler_state(
        &self,
        py: Python<'_>,
        state: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let state: SchedulerStateText = scheduler_state(state)?;
        {
            let mut callback_state = self.state();
            callback_state.scheduler_count += 1;
            if callback_state.scheduler_count > SCHEDULER_DIAGNOSTIC_LIMIT {
                callback_state.scheduler_omitted += 1;
                callback_state.last_scheduler = Some(state);
                return Ok(());
            }
        }
        self.write_runtime_diagnostic(py, &scheduler_diagnostic_message(state))
    }

    /// Report a bounded sample of completed warehouse statement identities.
    pub(crate) fn on_statement_complete(
        &self,
        py: Python<'_>,
        telemetry: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let telemetry: StatementText = statement(telemetry)?;
        {
            let mut state = self.state();
            let count: &mut usize = state
                .query_counts
                .entry(telemetry.resource_name.clone())
                .or_default();
            *count += 1;
            if *count > QUERY_DIAGNOSTIC_LIMIT {
                state.query_omitted += 1;
                state.last_query = Some(telemetry);
                return Ok(());
            }
        }
        self.write_runtime_diagnostic(py, &query_diagnostic_message(&telemetry))
    }

    /// Erase the live spinner row so a persistent line can take its place.
    pub(crate) fn clear_transient_line(&self, py: Python<'_>) -> PyResult<()> {
        self.locked(py, || self.emit(py, CLEAR_LINE))
    }

    /// Draw the live spinner row again below persistent output.
    pub(crate) fn redraw_transient_line(&self, py: Python<'_>) -> PyResult<()> {
        self.write_spinner_line(py)
    }

    pub(crate) fn on_node_complete(
        slf: &Bound<'_, Self>,
        node_result: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let py: Python<'_> = slf.py();
        let callbacks: &Self = slf.get();
        let identity: Option<(String, String)> = resource_identity(node_result)?;
        if let Some((name, _)) = &identity {
            callbacks.state().heartbeat.running.remove(name);
        }
        if let Some(worker) = &callbacks.worker
            && let Some((_, key)) = &identity
        {
            worker.completed(node_result, key)?;
        }
        let kind: ResultKind = result_kind(node_result)?;
        let mut result: Bound<'_, PyAny> = node_result.clone();
        if let Some(projector) = &callbacks.projector
            && let Some((name, identity)) = resource_identity(node_result)?
        {
            let Some(duration_ms) = consume_terminal(projector.bind(py), &name, Some(&identity))?
            else {
                return Ok(());
            };
            if matches!(
                kind,
                ResultKind::Model | ResultKind::Seed | ResultKind::Function | ResultKind::Load
            ) {
                let options: Bound<'_, PyDict> = PyDict::new(py);
                options.set_item("duration_ms", duration_ms.trunc() as i64)?;
                result = replaced_keywords(node_result, &options)?;
            }
        }
        let written: PyResult<()> = Self::write_node_result(slf, &result, kind);
        let evented: PyResult<()> = callbacks.write_event(py, &result);
        written.and(evented)
    }

    /// Write one result to the shared structured event channel.
    pub(crate) fn write_execution_event(
        &self,
        py: Python<'_>,
        result: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        self.write_event(py, result)
    }

    pub(crate) fn on_check_start(&self, py: Python<'_>, name: &str) -> PyResult<()> {
        if let Some(worker) = &self.worker {
            worker.start_check(py, name)?;
        }
        Ok(())
    }

    pub(crate) fn on_check_complete(
        &self,
        py: Python<'_>,
        result: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        if let Some(worker) = &self.worker {
            worker.completed_check(result)?;
        }
        self.write_event(py, result)
    }

    /// Stop the heartbeat and close the optional structured execution event stream.
    pub fn close(&self, py: Python<'_>) -> PyResult<()> {
        heartbeat::stop(py, self);
        let (scheduler, query) = {
            let state = self.state();
            (
                state
                    .last_scheduler
                    .map(|last| (state.scheduler_omitted, last)),
                state
                    .last_query
                    .clone()
                    .map(|last| (state.query_omitted, last)),
            )
        };
        for line in runtime_diagnostic_summary(
            scheduler,
            query.as_ref().map(|(omitted, last)| (*omitted, last)),
        ) {
            self.write_runtime_diagnostic(py, &line)?;
        }
        self.event_writer.bind(py).call_method0("close")?;
        Ok(())
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
            self.write_spinner_line(py)?;
        }
        Ok(())
    }
}

/// Render one live spinner row for all running items and grouped completed rows.
#[pyclass(module = "sqlbuild._native", frozen)]
pub struct NestedCommandProgressCallbacks {
    pub(crate) stream: Py<PyAny>,
    pub(crate) lines: Py<PyAny>,
    pub(crate) lock: Py<PyAny>,
    pub(crate) label: String,
    pub(crate) name_width: usize,
    pub(crate) style: CliStyle,
    pub(crate) is_tty: bool,
    pub(crate) sequential: bool,
    pub(crate) projector: Option<Py<PyAny>>,
    pub(crate) state: Mutex<crate::build_progress::_helpers::nested_progress::ReporterState>,
}

impl std::fmt::Debug for NestedCommandProgressCallbacks {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("NestedCommandProgressCallbacks")
            .finish_non_exhaustive()
    }
}

impl NestedCommandProgressCallbacks {
    pub(crate) fn state(
        &self,
    ) -> MutexGuard<'_, crate::build_progress::_helpers::nested_progress::ReporterState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub(crate) fn locked<T>(
        &self,
        py: Python<'_>,
        body: impl FnOnce() -> PyResult<T>,
    ) -> PyResult<T> {
        within(self.lock.bind(py), |_| body())
    }

    pub(crate) fn layout(&self, py: Python<'_>) -> PyResult<NestedLayout> {
        Ok(NestedLayout {
            label: self.label.clone(),
            name_width: self.name_width,
            columns: terminal_columns(py)?,
        })
    }

    pub(crate) fn draw_spinner_line(&self, py: Python<'_>) -> PyResult<()> {
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

    pub(crate) fn clear_live_row(&self, py: Python<'_>) -> PyResult<()> {
        if self.is_tty && self.state().line_active {
            write(self.stream.bind(py), CLEAR_LINE)?;
        }
        Ok(())
    }

    pub(crate) fn write_group_header(&self, py: Python<'_>, group: &str) -> PyResult<()> {
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

    pub(crate) fn set_cursor(&self, py: Python<'_>, hidden: bool) -> PyResult<()> {
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
    pub(crate) fn refresh_live_row(slf: &Bound<'_, Self>) -> PyResult<Option<Py<PyAny>>> {
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
    pub fn new(py: Python<'_>, options: Option<Bound<'_, PyDict>>) -> PyResult<Self> {
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
            state: Mutex::new(
                crate::build_progress::_helpers::nested_progress::ReporterState::default(),
            ),
        })
    }

    #[pyo3(signature = (**options))]
    pub(crate) fn on_item_start(
        slf: &Bound<'_, Self>,
        options: Option<Bound<'_, PyDict>>,
    ) -> PyResult<()> {
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
    pub(crate) fn on_item_complete(
        slf: &Bound<'_, Self>,
        options: Option<Bound<'_, PyDict>>,
    ) -> PyResult<()> {
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
    pub(crate) fn clear_transient_line(&self, py: Python<'_>) -> PyResult<()> {
        self.locked(py, || {
            let stream: &Bound<'_, PyAny> = self.stream.bind(py);
            write(stream, CLEAR_LINE)?;
            flush(stream)
        })
    }

    /// Draw the live spinner row again below persistent output.
    pub(crate) fn redraw_transient_line(&self, py: Python<'_>) -> PyResult<()> {
        self.locked(py, || {
            if self.state().line_active {
                self.draw_spinner_line(py)?;
            }
            Ok(())
        })
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
                if !stop_event.call_method0("is_set")?.is_truthy()? && self.state().line_active {
                    self.draw_spinner_line(py)?;
                }
                Ok(())
            })?;
        }
        Ok(())
    }
}

/// Render simple connection start, success, and failure messages.
#[pyclass(module = "sqlbuild._native", frozen)]
#[derive(Debug)]
pub struct ConnectionProgressReporter {
    pub(crate) adapter_name: String,
    pub(crate) stream: Py<PyAny>,
    pub(crate) blank_line_before_start: bool,
    pub(crate) blank_line_after_complete: bool,
    pub(crate) style: CliStyle,
}

impl ConnectionProgressReporter {
    pub(crate) fn write(&self, py: Python<'_>, message: &str) -> PyResult<()> {
        write_persistent(py, self.stream.bind(py), &format!("{message}\n"))
    }
}

#[pymethods]
impl ConnectionProgressReporter {
    #[new]
    #[pyo3(signature = (**options))]
    pub fn new(py: Python<'_>, options: Option<Bound<'_, PyDict>>) -> PyResult<Self> {
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

    pub(crate) fn on_connection_start(
        &self,
        py: Python<'_>,
        connection_count: i64,
    ) -> PyResult<()> {
        if self.blank_line_before_start {
            self.write(py, "")?;
        }
        let message: String = connection_start_message(&self.adapter_name, connection_count);
        self.write(py, &self.style.muted(&message))
    }

    #[pyo3(signature = (*, connection_count, elapsed_seconds))]
    pub(crate) fn on_connection_complete(
        &self,
        py: Python<'_>,
        connection_count: i64,
        elapsed_seconds: f64,
    ) -> PyResult<()> {
        publish_invocation_phase(
            py,
            &InvocationPhase {
                phase: CONNECTED_PHASE,
                duration_ms: Some(elapsed_seconds * 1000.0),
                count: Some(connection_count),
                name: Some(self.adapter_name.clone()),
                detail: None,
            },
        )?;
        let line: String =
            connection_phase_line(self.style, true, (&self.adapter_name, elapsed_seconds));
        self.write(py, &line)?;
        if self.blank_line_after_complete {
            self.write(py, "")?;
        }
        Ok(())
    }

    #[pyo3(signature = (*, connection_count, elapsed_seconds))]
    pub(crate) fn on_connection_error(
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
#[derive(Debug)]
pub struct PlanningProgressReporter {
    pub(crate) status: Py<PyAny>,
    pub(crate) active: AtomicBool,
    pub(crate) projector: Option<Py<NativeProgressProjector>>,
}

impl PlanningProgressReporter {
    pub(crate) fn call_status(&self, py: Python<'_>, method: &str, message: &str) -> PyResult<()> {
        self.status.bind(py).call_method1(method, (message,))?;
        Ok(())
    }

    pub(crate) fn call_status_keyword(
        &self,
        py: Python<'_>,
        method: &str,
        message: &str,
    ) -> PyResult<()> {
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
    pub fn new(py: Python<'_>, stream: &Bound<'_, PyAny>, use_color: bool) -> PyResult<Self> {
        let status: Bound<'_, PyAny> = status_reporter(stream, use_color)?;
        Ok(Self {
            status: status.unbind(),
            active: AtomicBool::new(false),
            projector: current_projector(py)?,
        })
    }

    pub fn on_progress(&self, py: Python<'_>, message: &str) -> PyResult<()> {
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

    pub fn start(&self, py: Python<'_>, message: &str) -> PyResult<()> {
        self.call_status(py, "start", message)?;
        self.active.store(true, Ordering::SeqCst);
        Ok(())
    }

    pub(crate) fn update(&self, py: Python<'_>, message: &str) -> PyResult<()> {
        if !self.active.load(Ordering::SeqCst) {
            return self.start(py, message);
        }
        self.call_status(py, "update", message)
    }

    pub fn complete(&self, py: Python<'_>, message: &str) -> PyResult<()> {
        self.call_status_keyword(py, "complete", message)?;
        self.active.store(false, Ordering::SeqCst);
        Ok(())
    }

    /// Complete with a pre-styled message written verbatim.
    pub fn complete_styled(&self, py: Python<'_>, message: &str) -> PyResult<()> {
        self.call_status_keyword(py, "complete_styled", message)?;
        self.active.store(false, Ordering::SeqCst);
        Ok(())
    }

    /// Close active progress and render a non-success terminal message.
    pub(crate) fn error(&self, py: Python<'_>, message: &str) -> PyResult<()> {
        self.call_status(py, "error", message)?;
        self.active.store(false, Ordering::SeqCst);
        Ok(())
    }

    #[pyo3(signature = (*, blank_line_after = false))]
    pub fn finish(&self, py: Python<'_>, blank_line_after: bool) -> PyResult<()> {
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
#[derive(Debug)]
pub struct DbtInitProgressReporter {
    pub(crate) stream: Py<PyAny>,
    pub(crate) status: Py<PyAny>,
    pub(crate) started: AtomicBool,
    pub(crate) phase_start: Mutex<Option<Instant>>,
}

#[pymethods]
impl DbtInitProgressReporter {
    #[new]
    #[pyo3(signature = (*, stream, use_color))]
    pub fn new(stream: &Bound<'_, PyAny>, use_color: bool) -> PyResult<Self> {
        Ok(Self {
            stream: stream.clone().unbind(),
            status: status_reporter(stream, use_color)?.unbind(),
            started: AtomicBool::new(false),
            phase_start: Mutex::new(None),
        })
    }

    pub(crate) fn start(&self, py: Python<'_>, message: &str) -> PyResult<()> {
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

    pub(crate) fn complete(&self, py: Python<'_>, message: &str) -> PyResult<()> {
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
