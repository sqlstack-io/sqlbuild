//! Envelope field validation, JSON freezing and catalogued event validation.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::pyfunction;
use pyo3::types::{PyBool, PyInt, PyString};
use sqlbuild_core::lifecycle_events::main::validate_known_event::validate_known_event;
use sqlbuild_core::lifecycle_events::models::KnownEventFacts;

use crate::lifecycle_events::_helpers::payloads::PathSegment;
use crate::lifecycle_events::_helpers::payloads::frozen;
use crate::lifecycle_events::_helpers::payloads::object_entries;
use crate::lifecycle_events::_helpers::payloads::validation_error;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

const CORRELATION_FIELDS: [&str; 5] = [
    "run_id",
    "resource_id",
    "resource_attempt_id",
    "operation_id",
    "statement_id",
];

/// Validate the shared positive integer schema-version contract.
#[pyfunction(name = "validate_lifecycle_schema_version")]
#[pyo3(signature = (*, value))]
pub(crate) fn schema_version_check(value: &Bound<'_, PyAny>) -> PyResult<()> {
    if value.is_instance_of::<PyBool>() || !value.is_instance_of::<PyInt>() || value.le(0)? {
        return Err(validation_error(
            value.py(),
            "schema_version must be a positive integer excluding bool",
        ));
    }
    Ok(())
}

/// Validate and recursively freeze a JSON-compatible value.
#[pyfunction(name = "freeze_lifecycle_json")]
#[pyo3(signature = (*, value, path))]
pub(crate) fn frozen_json<'py>(
    value: &Bound<'py, PyAny>,
    path: &str,
) -> PyResult<Bound<'py, PyAny>> {
    frozen(value, &[PathSegment::Key(path.to_owned())])
}

/// Validate one required non-empty string field.
#[pyfunction(name = "validate_lifecycle_required_text")]
#[pyo3(signature = (*, value, field_name))]
pub(crate) fn required_text_check(value: &Bound<'_, PyAny>, field_name: &str) -> PyResult<()> {
    let valid: bool = value
        .downcast::<PyString>()
        .is_ok_and(|text| text.len().is_ok_and(|length| length > 0));
    if !valid {
        return Err(validation_error(
            value.py(),
            &format!("{field_name} must be a non-empty string"),
        ));
    }
    Ok(())
}

/// Validate one optional non-empty string field.
#[pyfunction(name = "validate_lifecycle_optional_text")]
#[pyo3(signature = (*, value, field_name))]
pub(crate) fn optional_text_check(value: &Bound<'_, PyAny>, field_name: &str) -> PyResult<()> {
    if value.is_none() {
        return Ok(());
    }
    required_text_check(value, field_name)
}

/// Validate a timezone-aware UTC timestamp.
pub(crate) fn timestamp_check(value: &Bound<'_, PyAny>) -> PyResult<()> {
    let py = value.py();
    let datetime: Bound<'_, PyAny> = imported(py, "datetime", "datetime")?;
    if !value.is_instance(&datetime)? || native_field(value, "tzinfo")?.is_none() {
        return Err(validation_error(
            py,
            "occurred_at must be a timezone-aware UTC datetime",
        ));
    }
    let utc_offset: Bound<'_, PyAny> =
        imported(py, "datetime", "UTC")?.call_method1("utcoffset", (value,))?;
    if value.call_method0("utcoffset")?.ne(utc_offset)? {
        return Err(validation_error(py, "occurred_at must use UTC"));
    }
    Ok(())
}

/// Validate correlations and safe payload fields for a catalogued event.
pub(crate) fn known_event_check(event: &Bound<'_, PyAny>) -> PyResult<()> {
    let mut correlations: Vec<(&'static str, bool)> = Vec::new();
    for field in CORRELATION_FIELDS {
        correlations.push((field, !native_field(event, field)?.is_none()));
    }
    let facts = KnownEventFacts {
        schema_version: native_field(event, "schema_version")?.extract()?,
        event_type: native_field(event, "event_type")?.extract()?,
        correlations,
        payload: object_entries(&native_field(event, "payload")?)?,
    };
    validate_known_event(&facts).map_err(|message| validation_error(event.py(), &message))
}
