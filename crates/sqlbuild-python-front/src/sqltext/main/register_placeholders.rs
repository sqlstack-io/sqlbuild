//! Register the placeholders functions and classes.

use crate::sqltext::_helpers::placeholders;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the placeholders functions and classes.
#[inline]
pub fn register_placeholders(module: &Bound<'_, PyModule>) -> PyResult<()> {
    placeholders::register(module)
}
