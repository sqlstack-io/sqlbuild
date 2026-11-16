//! Register the refs functions and classes.

use crate::planner_resolution::_helpers::references::refs;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the refs functions and classes.
#[inline]
pub fn register_refs(module: &Bound<'_, PyModule>) -> PyResult<()> {
    refs::register(module)
}
