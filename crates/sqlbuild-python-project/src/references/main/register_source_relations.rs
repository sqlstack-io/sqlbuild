//! Register the source relations functions and classes.

use crate::references::_helpers::source_relations;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the source relations functions and classes.
#[inline]
pub fn register_source_relations(module: &Bound<'_, PyModule>) -> PyResult<()> {
    source_relations::register(module)
}
