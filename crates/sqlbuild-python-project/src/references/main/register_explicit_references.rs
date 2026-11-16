//! Register the explicit references functions and classes.

use crate::references::_helpers::explicit_references;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the explicit references functions and classes.
#[inline]
pub fn register_explicit_references(module: &Bound<'_, PyModule>) -> PyResult<()> {
    explicit_references::register(module)
}
