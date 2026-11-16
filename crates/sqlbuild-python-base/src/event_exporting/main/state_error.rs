//! `EventExporterStateError(message)`.

use crate::event_exporting::_helpers::values;
use pyo3::prelude::PyErr;
use pyo3::prelude::Python;

/// `EventExporterStateError(message)`.
#[inline]
pub fn state_error(py: Python<'_>, message: &str) -> PyErr {
    values::state_error(py, message)
}
