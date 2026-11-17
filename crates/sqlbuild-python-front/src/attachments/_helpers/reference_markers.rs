//! Reject executable SQL that still contains an unresolved reference call.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::{PyAnyMethods, PyErr, PyResult, Python};

/// The reference function and diagnostic code of each marker kind, in the scanner's order.
pub(crate) const MARKER_KINDS: [(&str, &str); 5] = [
    ("__ref", "R001"),
    ("__seed", "R002"),
    ("__source", "R003"),
    ("__udf", "R004"),
    ("__table_fn", "R005"),
];

/// A `ValueError` carrying `message` and `code` attributes.
pub(crate) fn coded_value_error(py: Python<'_>, message: String, code: &str) -> PyResult<PyErr> {
    let error: PyErr = PyValueError::new_err(message.clone());
    let value = error.value(py);
    value.setattr("message", message)?;
    value.setattr("code", code)?;
    Ok(error)
}
