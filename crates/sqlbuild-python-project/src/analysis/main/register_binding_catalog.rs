//! Register the binding catalog functions and classes.

use crate::analysis::_helpers::binding_catalog;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the binding catalog functions and classes.
#[inline]
pub fn register_binding_catalog(module: &Bound<'_, PyModule>) -> PyResult<()> {
    binding_catalog::register(module)
}
