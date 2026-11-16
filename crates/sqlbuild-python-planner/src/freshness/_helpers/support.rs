//! Python objects, exceptions and datetimes the source freshness glue shares.

use crate::freshness::main::presumed_utc_datetime::presumed_utc_datetime;
use pyo3::exceptions::PyException;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::PyString;

use crate::freshness::_helpers::exception_types::{
    SourceFreshnessInputError, SourceFreshnessObservationError,
};
use sqlbuild_python_front::state_rows::main::event_rows::date_time_typed::date_time_typed;

pub(crate) const OBSERVATION_ERROR: fn(String) -> PyErr = SourceFreshnessObservationError::new_err;
pub(crate) const INPUT_ERROR: fn(String) -> PyErr = SourceFreshnessInputError::new_err;

/// The native error `class(message)` as a raisable error, with an optional `__cause__`.
pub(crate) fn freshness_error(
    py: Python<'_>,
    (class, message): (fn(String) -> PyErr, &str),
    cause: Option<PyErr>,
) -> PyErr {
    let error: PyErr = class(message.to_owned());
    if let Some(cause) = cause {
        error.set_cause(py, Some(cause));
    }
    error
}

/// `str(exc)` of an `Exception`, or the error itself for anything else.
pub(crate) fn exception_text(py: Python<'_>, error: &PyErr) -> PyResult<Option<String>> {
    if !error.is_instance_of::<PyException>(py) {
        return Ok(None);
    }
    Ok(Some(error.value(py).str()?.extract()?))
}

/// `value` when it is not a `datetime`; otherwise in UTC, a naive value read as UTC.
pub(crate) fn presumed_utc_value<'py>(value: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    if !date_time_typed(value)? {
        return Ok(value.clone());
    }
    presumed_utc_datetime(value)
}

pub(crate) fn text<'py>(py: Python<'py>, value: &str) -> Bound<'py, PyAny> {
    PyString::new(py, value).into_any()
}

/// `str(value)`, or `None` for `None`.
pub(crate) fn optional_str<'py>(value: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    if value.is_none() {
        return Ok(value.clone());
    }
    Ok(value.str()?.into_any())
}
