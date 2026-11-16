//! Register the relationship names functions and classes.

use crate::scopes::_helpers::relationships::relationship_names;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the relationship names functions and classes.
#[inline]
pub fn register_relationship_names(module: &Bound<'_, PyModule>) -> PyResult<()> {
    relationship_names::register(module)
}
