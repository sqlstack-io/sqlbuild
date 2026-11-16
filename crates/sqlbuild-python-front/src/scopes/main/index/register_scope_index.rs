//! Register the scope index functions and classes.

use crate::scopes::_helpers::index::scope_index;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the scope index functions and classes.
#[inline]
pub fn register_scope_index(module: &Bound<'_, PyModule>) -> PyResult<()> {
    scope_index::register(module)
}
