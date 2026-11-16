//! Register the declaration contexts functions and classes.

use crate::scopes::_helpers::declarations::declaration_contexts;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the declaration contexts functions and classes.
#[inline]
pub fn register_declaration_contexts(module: &Bound<'_, PyModule>) -> PyResult<()> {
    declaration_contexts::register(module)
}
