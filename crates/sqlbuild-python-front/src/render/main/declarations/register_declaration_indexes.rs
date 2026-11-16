//! Register the declaration indexes functions and classes.

use crate::render::_helpers::declarations::declaration_indexes;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the declaration indexes functions and classes.
#[inline]
pub fn register_declaration_indexes(module: &Bound<'_, PyModule>) -> PyResult<()> {
    declaration_indexes::register(module)
}
