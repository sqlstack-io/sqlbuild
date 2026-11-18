//! Lifecycle event and diagnostic log envelopes, run bounds and operation field rules.

use pyo3::exceptions::PyTypeError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyBool, PyByteArray, PyBytes, PyDict, PyDictMethods, PyInt, PyTypeMethods};
use sqlbuild_core::lifecycle_events::constants::INVOCATION_SEQUENCE_PROBLEM;
use sqlbuild_core::lifecycle_events::main::diagnostic_severity_problem::diagnostic_severity_problem;
use sqlbuild_core::lifecycle_events::main::diagnostic_version_problem::diagnostic_version_problem;
use sqlbuild_core::lifecycle_events::main::hook_problem::hook_problem;
use sqlbuild_core::lifecycle_events::main::lifecycle_version_problem::lifecycle_version_problem;
use sqlbuild_core::lifecycle_events::main::operation_metadata_field::operation_metadata_field;
use sqlbuild_core::lifecycle_events::main::producer_version_problem::producer_version_problem;
use sqlbuild_core::lifecycle_events::main::publishable_hook_name::publishable_hook_name;
use sqlbuild_core::lifecycle_events::main::run_bounds_problem::run_bounds_problem;
use sqlbuild_core::lifecycle_events::models::{HookFacts, SequenceFact};
use sqlbuild_python_base::boundary::main::optional_text::optional_text;

use crate::lifecycle_events::_helpers::payloads::validation_error;
use crate::lifecycle_events::_helpers::validation::frozen_json;
use crate::lifecycle_events::_helpers::validation::known_event_check;
use crate::lifecycle_events::_helpers::validation::optional_text_check;
use crate::lifecycle_events::_helpers::validation::required_text_check;
use crate::lifecycle_events::_helpers::validation::schema_version_check;
use crate::lifecycle_events::_helpers::validation::timestamp_check;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_items::mapping_items;

const EVENT_REQUIRED_TEXT: [&str; 5] = [
    "event_id",
    "event_type",
    "producer",
    "producer_version",
    "invocation_id",
];
const EVENT_OPTIONAL_TEXT: [&str; 5] = [
    "run_id",
    "resource_id",
    "resource_attempt_id",
    "operation_id",
    "statement_id",
];
const DIAGNOSTIC_REQUIRED_TEXT: [&str; 6] = [
    "producer",
    "producer_version",
    "severity",
    "logger",
    "source",
    "message",
];
const DIAGNOSTIC_OPTIONAL_TEXT: [&str; 7] = [
    "log_stream_id",
    "invocation_id",
    "run_id",
    "resource_id",
    "resource_attempt_id",
    "operation_id",
    "statement_id",
];

/// Whether a value is an `int` other than a `bool`.
fn plain_int(value: &Bound<'_, PyAny>) -> bool {
    !value.is_instance_of::<PyBool>() && value.is_instance_of::<PyInt>()
}

/// Whether a value is a non-negative `int` other than a `bool`.
fn nonnegative_int(value: &Bound<'_, PyAny>) -> PyResult<bool> {
    Ok(plain_int(value) && !value.lt(0)?)
}

/// An integer, saturated into `i128`.
fn saturated(value: &Bound<'_, PyAny>) -> PyResult<i128> {
    match value.extract::<i128>() {
        Ok(number) => Ok(number),
        Err(_) if value.lt(0)? => Ok(i128::MIN),
        Err(_) => Ok(i128::MAX),
    }
}

/// Freeze a JSON object field of a frozen dataclass in place.
fn freeze_object_field(owner: &Bound<'_, PyAny>, field: &str, path: &str) -> PyResult<()> {
    let py: Python<'_> = owner.py();
    let frozen: Bound<'_, PyAny> = frozen_json(&native_field(owner, field)?, path)?;
    if !frozen.is_instance(&imported(py, "collections.abc", "Mapping")?)? {
        return Err(validation_error(
            py,
            &format!("{path} must be a JSON object"),
        ));
    }
    imported(py, "builtins", "object")?
        .getattr("__setattr__")?
        .call1((owner, field, frozen))?;
    Ok(())
}

/// Validate and freeze one lifecycle event envelope in place.
#[pyfunction(name = "validate_lifecycle_event")]
#[pyo3(signature = (*, event))]
pub(crate) fn lifecycle_event_check(event: &Bound<'_, PyAny>) -> PyResult<()> {
    let py: Python<'_> = event.py();
    for field in EVENT_REQUIRED_TEXT {
        required_text_check(&native_field(event, field)?, field)?;
    }
    let schema_version: Bound<'_, PyAny> = native_field(event, "schema_version")?;
    schema_version_check(&schema_version)?;
    let sequence: Bound<'_, PyAny> = native_field(event, "invocation_sequence")?;
    let sequence: SequenceFact = if sequence.is_none() {
        SequenceFact::Absent
    } else if nonnegative_int(&sequence)? {
        SequenceFact::Valid
    } else {
        SequenceFact::Invalid
    };
    let has_external_context: bool = native_field(event, "external_context")?.is_truthy()?;
    if let Some(problem) =
        lifecycle_version_problem(saturated(&schema_version)?, sequence, has_external_context)
    {
        return Err(validation_error(py, &problem));
    }
    timestamp_check(&native_field(event, "occurred_at")?)?;
    for field in EVENT_OPTIONAL_TEXT {
        optional_text_check(&native_field(event, field)?, field)?;
    }
    freeze_object_field(event, "external_context", "external_context")?;
    freeze_object_field(event, "payload", "payload")?;
    known_event_check(event)
}

/// Validate a dispatcher-assigned invocation sequence.
#[pyfunction(name = "validate_lifecycle_invocation_sequence")]
#[pyo3(signature = (*, value))]
pub(crate) fn invocation_sequence_check(value: &Bound<'_, PyAny>) -> PyResult<()> {
    if !nonnegative_int(value)? {
        return Err(validation_error(value.py(), INVOCATION_SEQUENCE_PROBLEM));
    }
    Ok(())
}

/// Validate and freeze one diagnostic log record in place.
#[pyfunction(name = "validate_diagnostic_log")]
#[pyo3(signature = (*, record))]
pub(crate) fn diagnostic_log_check(record: &Bound<'_, PyAny>) -> PyResult<()> {
    let py: Python<'_> = record.py();
    let schema_version: Bound<'_, PyAny> = native_field(record, "schema_version")?;
    schema_version_check(&schema_version)?;
    if let Some(problem) = diagnostic_version_problem(saturated(&schema_version)?) {
        return Err(validation_error(py, &problem));
    }
    for field in DIAGNOSTIC_REQUIRED_TEXT {
        required_text_check(&native_field(record, field)?, field)?;
    }
    let severity: String = native_field(record, "severity")?.extract()?;
    if let Some(problem) = diagnostic_severity_problem(&severity) {
        return Err(validation_error(py, problem));
    }
    timestamp_check(&native_field(record, "occurred_at")?)?;
    for field in DIAGNOSTIC_OPTIONAL_TEXT {
        optional_text_check(&native_field(record, field)?, field)?;
    }
    freeze_object_field(record, "fields", "fields")
}

/// Validate a run's selection, concurrency and worker counts.
#[pyfunction(name = "validate_run_bounds")]
#[pyo3(signature = (*, selected_count, configured_concurrency, worker_count))]
pub(crate) fn run_bounds_check(
    selected_count: &Bound<'_, PyAny>,
    configured_concurrency: &Bound<'_, PyAny>,
    worker_count: &Bound<'_, PyAny>,
) -> PyResult<()> {
    let py: Python<'_> = selected_count.py();
    let values: [(&str, &Bound<'_, PyAny>); 3] = [
        ("selected_count", selected_count),
        ("configured_concurrency", configured_concurrency),
        ("worker_count", worker_count),
    ];
    for (field, value) in values {
        if !plain_int(value) {
            return Err(validation_error(
                py,
                &format!("{field} must be an integer excluding bool"),
            ));
        }
    }
    match run_bounds_problem(
        saturated(selected_count)?,
        saturated(configured_concurrency)?,
        saturated(worker_count)?,
    ) {
        Some(problem) => Err(validation_error(py, problem)),
        None => Ok(()),
    }
}

/// The allow-listed, non-negative operation metadata counts.
#[pyfunction]
#[pyo3(signature = (*, metadata))]
pub(crate) fn validated_operation_metadata<'py>(
    py: Python<'py>,
    metadata: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    let result: Bound<'py, PyDict> = PyDict::new(py);
    if metadata.is_none() {
        return Ok(result);
    }
    for key in metadata.try_iter()? {
        if !operation_metadata_field(optional_text(&key?)?.as_deref()) {
            return Err(validation_error(
                py,
                "operation metadata contains a non-allowlisted field",
            ));
        }
    }
    for item in mapping_items(metadata)?.try_iter()? {
        let (key, value): (Bound<'py, PyAny>, Bound<'py, PyAny>) = item?.extract()?;
        if !nonnegative_int(&value)? {
            return Err(validation_error(
                py,
                "operation metadata values must be nonnegative integers excluding bool",
            ));
        }
        result.set_item(key, value)?;
    }
    Ok(result)
}

/// Whether a hook name is publishable, raising like `re.fullmatch` for non-string names.
fn hook_name_publishable(name: &Bound<'_, PyAny>) -> PyResult<bool> {
    if let Some(name) = optional_text(name)? {
        return Ok(publishable_hook_name(&name));
    }
    if name.is_instance_of::<PyBytes>() || name.is_instance_of::<PyByteArray>() {
        return Err(PyTypeError::new_err(
            "cannot use a string pattern on a bytes-like object",
        ));
    }
    let type_name: String = name.get_type().name()?.extract()?;
    Err(PyTypeError::new_err(format!(
        "expected string or bytes-like object, got '{type_name}'"
    )))
}

/// The validated hook fields of an operation, empty when none are supplied.
#[pyfunction]
#[pyo3(signature = (*, hook_phase, hook_index, hook_type, hook_name))]
pub(crate) fn validated_operation_hook_fields<'py>(
    hook_phase: &Bound<'py, PyAny>,
    hook_index: &Bound<'py, PyAny>,
    hook_type: &Bound<'py, PyAny>,
    hook_name: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    let py: Python<'py> = hook_phase.py();
    let result: Bound<'py, PyDict> = PyDict::new(py);
    let supplied: bool = [hook_phase, hook_index, hook_type, hook_name]
        .iter()
        .any(|value| !value.is_none());
    if !supplied {
        return Ok(result);
    }
    let phase: Option<String> = optional_text(hook_phase)?;
    let hook_type_text: Option<String> = optional_text(hook_type)?;
    let facts = HookFacts {
        phase: phase.as_deref(),
        hook_type: hook_type_text.as_deref(),
        valid_index: nonnegative_int(hook_index)?,
    };
    if let Some(problem) = hook_problem(facts) {
        return Err(validation_error(py, problem));
    }
    result.set_item("hook_phase", hook_phase)?;
    result.set_item("hook_index", hook_index)?;
    result.set_item("hook_type", hook_type)?;
    if !hook_name.is_none() && hook_name_publishable(hook_name)? {
        result.set_item("hook_name", hook_name)?;
    }
    Ok(result)
}

/// Validate that a producer other than SQLBuild names its version.
#[pyfunction(name = "validate_lifecycle_producer")]
#[pyo3(signature = (*, producer, producer_version))]
pub(crate) fn producer_check(
    producer: &Bound<'_, PyAny>,
    producer_version: &Bound<'_, PyAny>,
) -> PyResult<()> {
    match producer_version_problem(
        optional_text(producer)?.as_deref(),
        !producer_version.is_none(),
    ) {
        Some(problem) => Err(validation_error(producer.py(), problem)),
        None => Ok(()),
    }
}
