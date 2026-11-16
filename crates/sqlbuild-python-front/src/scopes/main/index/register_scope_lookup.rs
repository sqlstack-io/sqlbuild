//! Register the scope lookup functions and classes.

use crate::scopes::_helpers::index::scope_lookup;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the scope lookup functions and classes.
#[inline]
pub fn register_scope_lookup(module: &Bound<'_, PyModule>) -> PyResult<()> {
    scope_lookup::register(module)
}
