//! The invocation-scoped native CLI projection of canonical lifecycle facts.

use std::sync::{Mutex, MutexGuard, PoisonError};

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyBool, PyDict, PyDictMethods, PyFloat, PyInt, PyMapping, PyString};
use pyo3::{pyclass, pyfunction, pymethods, wrap_pyfunction};
use sqlbuild_core::progress::main::claim_progress_terminal::claim_progress_terminal;
use sqlbuild_core::progress::main::close_progress_projection::close_progress_projection;
use sqlbuild_core::progress::main::consume_progress_event::consume_progress_event;
use sqlbuild_core::progress::main::machine_output_mode::machine_output_mode as output_mode;
use sqlbuild_core::progress::main::progress_terminal_duration::progress_terminal_duration;
use sqlbuild_core::progress::models::{
    OutputModeFacts, ProgressEvent, ProgressPayload, ProgressProjection, ProgressSettings,
};
use sqlbuild_core::terminal_events::models::TerminalRequest;

use crate::bindings::_helpers::build_progress::transient_lines::shared_transient_line_coordinator;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_get::mapping_get;
use sqlbuild_python_base::records::main::fields::mapping_get_or::mapping_get_or;
use sqlbuild_python_base::records::main::fields::mapping_items::mapping_items;

/// The module attribute holding the `ContextVar` of the installed projector.
const CONTEXT_ATTRIBUTE: &str = "_native_progress_projector_context";
const CONTEXT_NAME: &str = "sqlbuild_native_progress_projector";

/// The `ContextVar` holding the installed projector.
static CONTEXT: PyOnceLock<Py<PyAny>> = PyOnceLock::new();
const CLAIM_OPTIONS: (&[&str], &[&str]) = (
    &["resource_name"],
    &["resource_id", "resource_attempt_id", "attempt_number"],
);
const DEFAULT_INTENT: &str = "execute";
const DEFAULT_STATEMENT_KIND: &str = "UNKNOWN";

/// Project canonical starts and correlate enriched terminal presentation.
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct NativeProgressProjector {
    state: Mutex<ProgressProjection>,
    stream: Py<PyAny>,
    coordinator: Py<PyAny>,
    /// The lines held back instead of written, while something holds them.
    held: Mutex<Option<Vec<String>>>,
}

fn payload_value<'py>(payload: &Bound<'py, PyAny>, key: &str) -> PyResult<Bound<'py, PyAny>> {
    mapping_get(payload, key)
}

fn text_value(value: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
    if value.is_instance_of::<PyString>() {
        return Ok(Some(value.extract()?));
    }
    Ok(None)
}

/// An `int` that is not a `bool`.
fn strict_integer(value: &Bound<'_, PyAny>) -> PyResult<Option<i64>> {
    if !value.is_instance_of::<PyInt>() || value.is_instance_of::<PyBool>() {
        return Ok(None);
    }
    Ok(Some(value.extract()?))
}

/// An `int` that is not a `bool`, as a number.
fn strict_integer_number(value: &Bound<'_, PyAny>) -> PyResult<Option<f64>> {
    if !value.is_instance_of::<PyInt>() || value.is_instance_of::<PyBool>() {
        return Ok(None);
    }
    Ok(Some(value.extract()?))
}

/// An `int`, including `bool`, which Python compares as one.
fn loose_integer(value: &Bound<'_, PyAny>) -> PyResult<Option<i64>> {
    if !value.is_instance_of::<PyInt>() {
        return Ok(None);
    }
    Ok(Some(value.extract()?))
}

fn number_value(value: &Bound<'_, PyAny>) -> PyResult<Option<f64>> {
    if value.is_instance_of::<PyInt>() || value.is_instance_of::<PyFloat>() {
        return Ok(Some(value.extract()?));
    }
    Ok(None)
}

/// `str(payload.get(key, default))`.
fn display_value(payload: &Bound<'_, PyAny>, key: &str, default: &str) -> PyResult<String> {
    let value: Bound<'_, PyAny> = mapping_get_or(payload, key, default)?;
    Ok(value.str()?.to_string())
}

fn progress_payload(payload: &Bound<'_, PyAny>) -> PyResult<ProgressPayload> {
    let metadata: Bound<'_, PyAny> = payload_value(payload, "metadata")?;
    let metadata_integer = |key: &str| -> PyResult<Option<i64>> {
        if metadata.downcast::<PyMapping>().is_err() {
            return Ok(None);
        }
        strict_integer(&payload_value(&metadata, key)?)
    };
    Ok(ProgressPayload {
        resource_name: text_value(&payload_value(payload, "resource_name")?)?,
        resource_kind: text_value(&payload_value(payload, "resource_kind")?)?,
        duration_ms: number_value(&payload_value(payload, "duration_ms")?)?,
        attempt_number: loose_integer(&payload_value(payload, "attempt_number")?)?,
        operation_name: text_value(&payload_value(payload, "operation_name")?)?,
        metadata_attempt_number: metadata_integer("attempt_number")?,
        metadata_item_count: metadata_integer("item_count")?,
        metadata_unknown_count: metadata_integer("unknown_count")?,
        error_type: text_value(&payload_value(payload, "error_type")?)?,
        failed_attempt_number: strict_integer(&payload_value(payload, "failed_attempt_number")?)?,
        next_attempt_number: strict_integer(&payload_value(payload, "next_attempt_number")?)?,
        delay_ms: strict_integer_number(&payload_value(payload, "delay_ms")?)?,
        query_id: text_value(&payload_value(payload, "query_id")?)?,
        phase: text_value(&payload_value(payload, "phase")?)?,
        intent: display_value(payload, "intent", DEFAULT_INTENT)?,
        statement_kind: display_value(payload, "statement_kind", DEFAULT_STATEMENT_KIND)?,
    })
}

fn optional_text(event: &Bound<'_, PyAny>, name: &str) -> PyResult<Option<String>> {
    native_field(event, name)?.extract()
}

/// The facts of one lifecycle event that progress projection reads.
fn progress_event(event: &Bound<'_, PyAny>) -> PyResult<ProgressEvent> {
    Ok(ProgressEvent {
        event_id: native_field(event, "event_id")?.extract()?,
        event_type: native_field(event, "event_type")?.extract()?,
        resource_id: optional_text(event, "resource_id")?,
        resource_attempt_id: optional_text(event, "resource_attempt_id")?,
        operation_id: optional_text(event, "operation_id")?,
        statement_id: optional_text(event, "statement_id")?,
        payload: progress_payload(&native_field(event, "payload")?)?,
    })
}

/// The coordinator shared by every terminal writer in this process.
pub(crate) fn shared_coordinator(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    shared_transient_line_coordinator(py)
        .and_then(|value| pyo3::IntoPyObjectExt::into_bound_py_any(value, py))
}

fn persist(coordinator: &Bound<'_, PyAny>, stream: &Bound<'_, PyAny>, text: &str) -> PyResult<()> {
    let keywords: Bound<'_, PyDict> = PyDict::new(coordinator.py());
    keywords.set_item("stream", stream)?;
    keywords.set_item("text", text)?;
    coordinator.call_method("write_persistent", (), Some(&keywords))?;
    Ok(())
}

/// Write lasting text, clearing and then redrawing any live transient line.
pub(crate) fn write_persistent(
    py: Python<'_>,
    stream: &Bound<'_, PyAny>,
    text: &str,
) -> PyResult<()> {
    persist(&shared_coordinator(py)?, stream, text)
}

pub(crate) fn terminal_stream(stream: &Bound<'_, PyAny>) -> PyResult<bool> {
    if !stream.hasattr("isatty")? {
        return Ok(false);
    }
    stream.call_method0("isatty")?.is_truthy()
}

impl NativeProgressProjector {
    fn held(&self) -> MutexGuard<'_, Option<Vec<String>>> {
        self.held.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Hold back every line from now on instead of writing it.
    pub(crate) fn hold_lines(&self) {
        *self.held() = Some(Vec::new());
    }

    /// Replace the lines held back so far with `lines`, returning them; nothing while not holding.
    pub(crate) fn exchange_held_lines(&self, lines: Vec<String>) -> Vec<String> {
        match self.held().as_mut() {
            Some(held) => std::mem::replace(held, lines),
            None => Vec::new(),
        }
    }

    /// Write lines as they come again, returning those held back since `hold_lines`.
    pub(crate) fn release_lines(&self) -> Vec<String> {
        self.held().take().unwrap_or_default()
    }

    /// Write lines held back earlier, as they would have been written.
    pub(crate) fn write_held(&self, py: Python<'_>, lines: &[String]) -> PyResult<()> {
        let coordinator: &Bound<'_, PyAny> = self.coordinator.bind(py);
        for line in lines {
            persist(coordinator, self.stream.bind(py), line)?;
        }
        Ok(())
    }

    fn locked(&self) -> PyResult<MutexGuard<'_, ProgressProjection>> {
        self.state
            .lock()
            .map_err(|_| PyRuntimeError::new_err("native progress projector lock is poisoned"))
    }

    /// Run `update` and write its lines while holding the shared terminal line lock.
    fn write_with<T>(
        &self,
        py: Python<'_>,
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
}

/// The terminal a result callback names through its keyword arguments.
fn terminal_request(
    py: Python<'_>,
    function: &str,
    options: Option<Bound<'_, PyDict>>,
) -> PyResult<TerminalRequest> {
    let arguments: KeywordArguments<'_> =
        KeywordArguments::read(py, function, options, CLAIM_OPTIONS)?;
    Ok(TerminalRequest {
        resource_name: arguments.value("resource_name")?.extract()?,
        resource_id: arguments.value("resource_id")?.extract()?,
        resource_attempt_id: arguments.value("resource_attempt_id")?.extract()?,
        attempt_number: arguments.value("attempt_number")?.extract()?,
    })
}

#[pymethods]
impl NativeProgressProjector {
    #[new]
    #[pyo3(signature = (*, stream, use_color, debug = false))]
    pub(crate) fn new(
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
        })
    }

    /// Install this projector for presentation adapters in the invocation context.
    fn install<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        context_variable(slf.py())?.call_method1("set", (slf,))
    }

    /// Restore the previous invocation projector.
    fn restore(&self, py: Python<'_>, token: &Bound<'_, PyAny>) -> PyResult<()> {
        context_variable(py)?.call_method1("reset", (token,))?;
        Ok(())
    }

    /// Add safe display order metadata without changing canonical facts.
    #[pyo3(signature = (*, ordinals, total))]
    fn configure_resources(&self, ordinals: &Bound<'_, PyAny>, total: i64) -> PyResult<()> {
        let mut entries: Vec<(String, i64)> = Vec::new();
        for item in mapping_items(ordinals)?.try_iter()? {
            entries.push(item?.extract()?);
        }
        self.locked()?.configure_resources(entries, total);
        Ok(())
    }

    /// Declare that an existing result callback will enrich this resource terminal.
    #[pyo3(signature = (*, resource_name))]
    fn expect_resource_enrichment(&self, resource_name: &str) -> PyResult<()> {
        self.locked()?.expect_resource_enrichment(resource_name);
        Ok(())
    }

    /// Record how one audit attempt ended, so its terminal row shows the outcome.
    #[pyo3(signature = (*, resource_attempt_id, outcome))]
    fn record_audit_outcome(&self, resource_attempt_id: &str, outcome: &str) -> PyResult<()> {
        self.locked()?
            .record_audit_outcome(resource_attempt_id, outcome);
        Ok(())
    }

    /// Return whether a visible canonical operation currently owns progress.
    #[pyo3(name = "is_operation_active", signature = (*, operation_name))]
    pub(crate) fn operation_active(&self, operation_name: &str) -> PyResult<bool> {
        Ok(self.locked()?.is_operation_active(operation_name))
    }

    /// Consume one known lifecycle fact idempotently.
    fn consume(&self, py: Python<'_>, event: &Bound<'_, PyAny>) -> PyResult<()> {
        let event: ProgressEvent = progress_event(event)?;
        self.write_with(py, |projection| {
            let (projection, lines) = consume_progress_event(projection, &event);
            (projection, lines, ())
        })
    }

    /// Claim the targeted or latest terminal for one final resource result.
    #[pyo3(signature = (**options))]
    fn consume_resource_terminal(
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
    fn resource_terminal_duration(
        &self,
        py: Python<'_>,
        options: Option<Bound<'_, PyDict>>,
    ) -> PyResult<Option<f64>> {
        let request: TerminalRequest = terminal_request(py, "resource_terminal_duration", options)?;
        Ok(progress_terminal_duration(&*self.locked()?, &request))
    }

    /// Render resource terminals no result callback claimed.
    fn close(&self, py: Python<'_>) -> PyResult<()> {
        self.write_with(py, |projection| {
            let (projection, lines) = close_progress_projection(projection);
            (projection, lines, ())
        })
    }
}

fn context_variable(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    Ok(CONTEXT
        .get_or_try_init(py, || {
            let keywords: Bound<'_, PyDict> = PyDict::new(py);
            keywords.set_item("default", py.None())?;
            Ok::<_, pyo3::PyErr>(
                py.import("contextvars")?
                    .getattr("ContextVar")?
                    .call((CONTEXT_NAME,), Some(&keywords))?
                    .unbind(),
            )
        })?
        .bind(py)
        .clone())
}

/// Return the projector installed for the current CLI invocation.
#[pyfunction]
pub(crate) fn current_native_progress_projector(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    context_variable(py)?.call_method0("get")
}

/// Whether the parsed command writes machine output to stdout, so progress must use stderr.
#[pyfunction]
pub(crate) fn machine_output_mode(args: &Bound<'_, PyAny>) -> PyResult<bool> {
    let mut dbt_args: Vec<String> = Vec::new();
    for arg in native_field(args, "dbt_args")?.try_iter()? {
        dbt_args.push(arg?.str()?.extract()?);
    }
    Ok(output_mode(&OutputModeFacts {
        json: native_field(args, "json")?.is_truthy()?,
        command: optional_text(args, "command")?,
        lineage_format: optional_text(args, "lineage_format")?,
        query_format: optional_text(args, "query_format")?,
        dbt_args,
    }))
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    let py: Python<'_> = module.py();
    module.add(CONTEXT_ATTRIBUTE, context_variable(py)?)?;
    module.add_class::<NativeProgressProjector>()?;
    module.add_function(wrap_pyfunction!(machine_output_mode, module)?)?;
    module.add_function(wrap_pyfunction!(current_native_progress_projector, module)?)
}

/// The projector installed for this invocation, when there is one.
pub(crate) fn current_projector(py: Python<'_>) -> PyResult<Option<Py<NativeProgressProjector>>> {
    let installed: Bound<'_, PyAny> = context_variable(py)?.call_method0("get")?;
    if installed.is_none() {
        return Ok(None);
    }
    Ok(Some(installed.extract()?))
}

/// Whether an installed projector shows a visible operation with this name.
pub(crate) fn projector_operation_active(
    projector: &NativeProgressProjector,
    operation_name: &str,
) -> PyResult<bool> {
    projector.operation_active(operation_name)
}
