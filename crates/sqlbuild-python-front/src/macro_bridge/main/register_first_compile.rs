//! Register the first compile functions and classes.

use crate::macro_bridge::_helpers::first_compile;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the first compile functions and classes.
#[inline]
pub fn register_first_compile(module: &Bound<'_, PyModule>) -> PyResult<()> {
    first_compile::register(module)
}
