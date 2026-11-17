//! Register the scope cache functions and classes.

use crate::scopes::_helpers::index::scope_cache;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the scope cache functions and classes.
#[inline]
pub fn register_scope_cache(module: &Bound<'_, PyModule>) -> PyResult<()> {
    scope_cache::register(module)
}
