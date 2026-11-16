//! Register the declaration values functions and classes.

use crate::render::_helpers::declarations::declaration_values;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the declaration values functions and classes.
#[inline]
pub fn register_declaration_values(module: &Bound<'_, PyModule>) -> PyResult<()> {
    declaration_values::register(module)
}
