//! The invocation-scoped native CLI projection of canonical lifecycle facts.

use crate::build_progress::models::NativeProgressProjector;

use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyBool, PyDict, PyDictMethods, PyFloat, PyInt, PyMapping, PyString};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_core::progress::main::machine_output_mode::machine_output_mode as output_mode;
use sqlbuild_core::progress::models::{OutputModeFacts, ProgressEvent, ProgressPayload};
use sqlbuild_core::terminal_events::models::TerminalRequest;

use crate::boundary::models::KeywordArguments;
use crate::build_progress::_helpers::transient_lines::shared_transient_line_coordinator;
use crate::records::main::fields::field::field as native_field;
use crate::records::main::fields::mapping_get::mapping_get;
use crate::records::main::fields::mapping_get_or::mapping_get_or;

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
        if metadata.cast::<PyMapping>().is_err() {
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
pub(crate) fn progress_event(event: &Bound<'_, PyAny>) -> PyResult<ProgressEvent> {
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

pub(crate) fn persist(
    coordinator: &Bound<'_, PyAny>,
    stream: &Bound<'_, PyAny>,
    text: &str,
) -> PyResult<()> {
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

/// The terminal a result callback names through its keyword arguments.
pub(crate) fn terminal_request(
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

pub(crate) fn context_variable(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
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
