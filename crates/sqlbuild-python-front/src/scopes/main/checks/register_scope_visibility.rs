//! Register the scope visibility functions and classes.

use crate::scopes::_helpers::checks::scope_visibility;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the scope visibility functions and classes.
#[inline]
pub fn register_scope_visibility(module: &Bound<'_, PyModule>) -> PyResult<()> {
    scope_visibility::register(module)
}
