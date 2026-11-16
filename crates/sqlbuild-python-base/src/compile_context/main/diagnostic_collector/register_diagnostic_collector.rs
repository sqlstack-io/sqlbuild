//! Register the diagnostic collector functions and classes.

use crate::compile_context::_helpers::diagnostic_collector;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the diagnostic collector functions and classes.
#[inline]
pub fn register_diagnostic_collector(module: &Bound<'_, PyModule>) -> PyResult<()> {
    diagnostic_collector::register(module)
}
