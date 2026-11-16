//! Register the source bindings functions and classes.

use crate::semantic_shapes::_helpers::source_bindings;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the source bindings functions and classes.
#[inline]
pub fn register_source_bindings(module: &Bound<'_, PyModule>) -> PyResult<()> {
    source_bindings::register(module)
}
