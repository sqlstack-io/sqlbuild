//! The sanitized class name of a raised Python exception.

use crate::event_exporting::_helpers::values;
use pyo3::prelude::PyErr;
use pyo3::prelude::Python;

/// The sanitized class name of a raised Python exception.
#[inline]
pub fn error_type_of(py: Python<'_>, error: &PyErr) -> String {
    values::error_type_of(py, error)
}
