//! Register the scope validation functions and classes.

use crate::scopes::_helpers::checks::scope_validation;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the scope validation functions and classes.
#[inline]
pub fn register_scope_validation(module: &Bound<'_, PyModule>) -> PyResult<()> {
    scope_validation::register(module)
}
