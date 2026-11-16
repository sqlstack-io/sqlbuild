//! Register the redaction functions and classes.

use crate::diagnostics::_helpers::redaction;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the redaction functions and classes.
#[inline]
pub fn register_redaction(module: &Bound<'_, PyModule>) -> PyResult<()> {
    redaction::register(module)
}
