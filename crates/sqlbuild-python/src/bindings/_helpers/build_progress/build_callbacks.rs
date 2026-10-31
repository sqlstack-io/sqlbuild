//! Live progress rows, completion rows and runtime diagnostics of `sqb build`.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Instant;

use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyCFunction, PyDict, PyDictMethods, PyTuple, PyTupleMethods};
use pyo3::{pyclass, pymethods};
use sqlbuild_core::build_progress::main::batch_progress_message::batch_progress_message;
use sqlbuild_core::build_progress::main::query_diagnostic_message::query_diagnostic_message;
use sqlbuild_core::build_progress::main::render_build_spinner_line::render_build_spinner_line;
use sqlbuild_core::build_progress::main::render_failed_test_result::render_failed_test_result;
use sqlbuild_core::build_progress::main::render_model_result::render_model_result;
use sqlbuild_core::build_progress::main::render_resource_result::render_resource_result;
use sqlbuild_core::build_progress::main::runtime_diagnostic_summary::runtime_diagnostic_summary;
use sqlbuild_core::build_progress::main::scheduler_diagnostic_message::scheduler_diagnostic_message;
use sqlbuild_core::build_progress::models::{
    BatchProgressText, ModelRowFacts, ProgressLayout, ResourceResult, SchedulerStateText,
    SpinnerRow, StatementText, TestResultText,
};
use sqlbuild_core::cli_text::models::CliStyle;

use crate::bindings::_helpers::boundary::python_calls::{KeywordArguments, imported, within};
use crate::bindings::_helpers::build_progress::plan_facts::{BuildPlanFacts, build_plan_facts};
use crate::bindings::_helpers::build_progress::result_text::{
    function_text, load_text, model_text, optional, seed_text, test_text, text,
};
use crate::bindings::_helpers::build_progress::terminal::{
    CLEAR_LINE, HIDE_CURSOR, SHOW_CURSOR, SPINNER_FRAMES, SPINNER_TICK_SECONDS, claim,
    consume_terminal, flush, join_spinner, projector, release, shared_lines, start_spinner,
    terminal_columns, tty_flag, write,
};
use crate::bindings::_helpers::compiled_project::snapshot_execution::WorkerBuild;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::fields::replaced_keywords;
use crate::bindings::_helpers::result_output::event_writer::ExecutionEventWriter;

const RUNTIME_LOGGER: &str = "sqlbuild.executor.runtime";
const PASS_OUTCOME: &str = "pass";
const SCHEDULER_DIAGNOSTIC_LIMIT: usize = 50;
const QUERY_DIAGNOSTIC_LIMIT: usize = 25;

/// Which executor result type a completion reports.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ResultKind {
    Model,
    Seed,
    Function,
    Load,
    PythonNode,
    SqlTest,
    Other,
}

fn result_kind(result: &Bound<'_, PyAny>) -> PyResult<ResultKind> {
    for (field, kind) in [
        ("model_name", ResultKind::Model),
        ("seed_name", ResultKind::Seed),
        ("function_name", ResultKind::Function),
        ("loader_name", ResultKind::Load),
        ("node_name", ResultKind::PythonNode),
        ("test_name", ResultKind::SqlTest),
    ] {
        if result.hasattr(field)? {
            return Ok(kind);
        }
    }
    Ok(ResultKind::Other)
}

fn sql_test_resource_id(result: &Bound<'_, PyAny>) -> PyResult<String> {
    let name: String = text(result, "test_name")?;
    let path: Bound<'_, PyAny> = native_field(result, "source_path")?;
    let block: Bound<'_, PyAny> = native_field(result, "block_index")?;
    let case: Bound<'_, PyAny> = native_field(result, "case_name")?;
    if path.is_none() || block.is_none() || case.is_none() {
        return Ok(format!("sql_test:{name}"));
    }
    Ok(format!(
        "sql_test:{}:{}:{}",
        path.call_method0("as_posix")?.str()?,
        block.str()?,
        case.str()?
    ))
}

/// The resource name and projector identity of a completion.
pub(crate) fn resource_identity(result: &Bound<'_, PyAny>) -> PyResult<Option<(String, String)>> {
    Ok(match result_kind(result)? {
        ResultKind::Model => {
            let name: String = text(result, "model_name")?;
            Some((name.clone(), format!("model:{name}")))
        }
        ResultKind::Seed => {
            let name: String = text(result, "seed_name")?;
            Some((name.clone(), format!("seed:{name}")))
        }
        ResultKind::Function => {
            let name: String = text(result, "function_name")?;
            let kind: String = text(result, "function_kind")?;
            Some((name.clone(), format!("{kind}:{name}")))
        }
        ResultKind::Load => {
            let name: String = text(result, "source_name")?;
            Some((name.clone(), format!("source:{name}")))
        }
        ResultKind::PythonNode => {
            let name: String = text(result, "node_name")?;
            let kind: String = text(result, "kind")?;
            Some((name.clone(), format!("{kind}:{name}")))
        }
        ResultKind::SqlTest => Some((text(result, "test_name")?, sql_test_resource_id(result)?)),
        ResultKind::Other => None,
    })
}

/// Mutable callback state; only held briefly and never across Python calls.
#[derive(Default)]
struct CallbackState {
    counter: usize,
    node_name: String,
    node_kind: String,
    sub_message: String,
    frame: usize,
    line_active: bool,
    cursor_hidden: bool,
    stop_event: Option<Py<PyAny>>,
    thread: Option<Py<PyAny>>,
    tests_by_model: HashMap<String, Vec<TestResultText>>,
    scheduler_count: usize,
    scheduler_omitted: usize,
    last_scheduler: Option<SchedulerStateText>,
    query_counts: HashMap<String, usize>,
    query_omitted: usize,
    last_query: Option<StatementText>,
}

/// Encapsulates live build progress output state and callbacks.
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct BuildProgressCallbacks {
    pub(crate) worker: Option<Arc<WorkerBuild>>,
    plan: Py<PyAny>,
    stream: Py<PyAny>,
    lines: Py<PyAny>,
    lock: Py<PyAny>,
    event_writer: Py<PyAny>,
    projector: Option<Py<PyAny>>,
    facts: BuildPlanFacts,
    style: CliStyle,
    verbose: bool,
    debug: bool,
    is_tty: bool,
    started: Instant,
    state: Mutex<CallbackState>,
}

impl BuildProgressCallbacks {
    fn state(&self) -> MutexGuard<'_, CallbackState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn locked<T>(&self, py: Python<'_>, body: impl FnOnce() -> PyResult<T>) -> PyResult<T> {
        within(self.lock.bind(py), |_| body())
    }

    fn layout(&self, py: Python<'_>) -> PyResult<ProgressLayout> {
        Ok(ProgressLayout {
            total: self.facts.total,
            name_width: self.facts.name_width,
            columns: terminal_columns(py)?,
            verbose: self.verbose,
        })
    }

    fn emit(&self, py: Python<'_>, text: &str) -> PyResult<()> {
        let stream: &Bound<'_, PyAny> = self.stream.bind(py);
        write(stream, text)?;
        flush(stream)
    }

    fn write_spinner_line(&self, py: Python<'_>) -> PyResult<()> {
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

    fn set_cursor(&self, py: Python<'_>, hidden: bool) -> PyResult<()> {
        if self.state().cursor_hidden == hidden {
            return Ok(());
        }
        self.locked(py, || {
            self.emit(py, if hidden { HIDE_CURSOR } else { SHOW_CURSOR })
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
            join_spinner(thread.bind(py))?;
        }
        Ok(())
    }

    /// Stop the spinner and erase its row before a persistent row is written.
    fn retire_live_row(slf: &Bound<'_, Self>) -> PyResult<()> {
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

    fn write_runtime_diagnostic(&self, py: Python<'_>, message: &str) -> PyResult<()> {
        imported(py, "logging", "getLogger")?
            .call1((RUNTIME_LOGGER,))?
            .call_method1("debug", (message,))?;
        if self.debug {
            return Ok(());
        }
        self.write_persistent(py, message)
    }

    /// Write one permanent line through the shared line coordinator.
    fn write_persistent(&self, py: Python<'_>, message: &str) -> PyResult<()> {
        let options: Bound<'_, PyDict> = PyDict::new(py);
        options.set_item("stream", self.stream.bind(py))?;
        options.set_item("text", format!("{message}\n"))?;
        self.lines
            .bind(py)
            .call_method("write_persistent", (), Some(&options))?;
        Ok(())
    }

    /// Keep a line per completed microbatch when no live spinner row shows batch progress.
    fn write_batch_complete(&self, py: Python<'_>, progress: &Bound<'_, PyAny>) -> PyResult<()> {
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

    fn write_node_result(
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

    fn write_event(&self, py: Python<'_>, result: &Bound<'_, PyAny>) -> PyResult<()> {
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

fn scheduler_state(state: &Bound<'_, PyAny>) -> PyResult<SchedulerStateText> {
    Ok(SchedulerStateText {
        running: native_field(state, "running")?.extract()?,
        ready: native_field(state, "ready")?.extract()?,
        waiting: native_field(state, "waiting")?.extract()?,
        limit: native_field(state, "limit")?.extract()?,
        aborted: native_field(state, "aborted")?.extract()?,
    })
}

fn statement(telemetry: &Bound<'_, PyAny>) -> PyResult<StatementText> {
    Ok(StatementText {
        query_id: optional(telemetry, "query_id")?,
        status: text(telemetry, "status")?,
        elapsed_seconds: native_field(telemetry, "elapsed_seconds")?.extract()?,
        resource_type: text(telemetry, "resource_type")?,
        resource_name: text(telemetry, "resource_name")?,
        phase: text(telemetry, "phase")?,
    })
}

#[pymethods]
impl BuildProgressCallbacks {
    #[new]
    #[pyo3(signature = (**options))]
    pub(crate) fn new(py: Python<'_>, options: Option<Bound<'_, PyDict>>) -> PyResult<Self> {
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
    pub(crate) fn elapsed(&self) -> f64 {
        self.started.elapsed().as_secs_f64()
    }

    #[pyo3(signature = (*, name, resource_kind))]
    fn on_node_start(
        slf: &Bound<'_, Self>,
        name: String,
        resource_kind: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let py: Python<'_> = slf.py();
        let callbacks: &Self = slf.get();
        if let Some(worker) = &callbacks.worker {
            worker.start(py, &name, resource_kind)?;
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

    fn on_sub_progress(&self, py: Python<'_>, message: String) -> PyResult<()> {
        self.state().sub_message = message;
        if self.is_tty {
            self.write_spinner_line(py)?;
        }
        Ok(())
    }

    /// Keep a line per completed microbatch when no live spinner row shows batch progress.
    fn on_batch_complete(&self, py: Python<'_>, progress: &Bound<'_, PyAny>) -> PyResult<()> {
        self.write_batch_complete(py, progress)
    }

    /// Report one deduplicated concurrent scheduler state transition.
    fn on_scheduler_state(&self, py: Python<'_>, state: &Bound<'_, PyAny>) -> PyResult<()> {
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
    fn on_statement_complete(&self, py: Python<'_>, telemetry: &Bound<'_, PyAny>) -> PyResult<()> {
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
    fn clear_transient_line(&self, py: Python<'_>) -> PyResult<()> {
        self.locked(py, || self.emit(py, CLEAR_LINE))
    }

    /// Draw the live spinner row again below persistent output.
    fn redraw_transient_line(&self, py: Python<'_>) -> PyResult<()> {
        self.write_spinner_line(py)
    }

    fn on_node_complete(slf: &Bound<'_, Self>, node_result: &Bound<'_, PyAny>) -> PyResult<()> {
        let py: Python<'_> = slf.py();
        let callbacks: &Self = slf.get();
        if let Some(worker) = &callbacks.worker
            && let Some((_, key)) = resource_identity(node_result)?
        {
            worker.completed(node_result, &key)?;
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
    fn write_execution_event(&self, py: Python<'_>, result: &Bound<'_, PyAny>) -> PyResult<()> {
        self.write_event(py, result)
    }

    /// Close the optional structured execution event stream.
    fn close(&self, py: Python<'_>) -> PyResult<()> {
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

/// A callable that forwards each completed microbatch to `callbacks`, built without a Python lookup.
pub(crate) fn batch_complete_callback<'py>(
    callbacks: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let owned: Py<BuildProgressCallbacks> = callbacks
        .downcast::<BuildProgressCallbacks>()?
        .clone()
        .unbind();
    let forward = move |arguments: &Bound<'_, PyTuple>,
                        _keywords: Option<&Bound<'_, PyDict>>|
          -> PyResult<()> {
        owned
            .get()
            .write_batch_complete(arguments.py(), &arguments.get_item(0)?)
    };
    Ok(PyCFunction::new_closure(callbacks.py(), None, None, forward)?.into_any())
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<BuildProgressCallbacks>()
}
