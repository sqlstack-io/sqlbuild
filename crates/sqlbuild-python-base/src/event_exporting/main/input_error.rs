//! `EventExporterInputError(message)`.

use crate::event_exporting::_helpers::values;
use pyo3::prelude::PyErr;
use pyo3::prelude::Python;

/// `EventExporterInputError(message)`.
#[inline]
pub fn input_error(py: Python<'_>, message: &str) -> PyErr {
    values::input_error(py, message)
}
