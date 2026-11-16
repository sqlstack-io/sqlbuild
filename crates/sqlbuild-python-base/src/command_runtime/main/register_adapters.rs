//! Register the adapters functions and classes.

use crate::command_runtime::_helpers::adapters;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the adapters functions and classes.
#[inline]
pub fn register_adapters(module: &Bound<'_, PyModule>) -> PyResult<()> {
    adapters::register(module)
}
