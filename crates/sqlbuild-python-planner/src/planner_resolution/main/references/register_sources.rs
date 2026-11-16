//! Register the sources functions and classes.

use crate::planner_resolution::_helpers::references::sources;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the sources functions and classes.
#[inline]
pub fn register_sources(module: &Bound<'_, PyModule>) -> PyResult<()> {
    sources::register(module)
}
