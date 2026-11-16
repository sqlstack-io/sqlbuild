//! Register the spec resolution functions and classes.

use crate::contracts::_helpers::spec_resolution;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the spec resolution functions and classes.
#[inline]
pub fn register_spec_resolution(module: &Bound<'_, PyModule>) -> PyResult<()> {
    spec_resolution::register(module)
}
