//! Register the relations functions and classes.

use crate::references::_helpers::relations;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the relations functions and classes.
#[inline]
pub fn register_relations(module: &Bound<'_, PyModule>) -> PyResult<()> {
    relations::register(module)
}
