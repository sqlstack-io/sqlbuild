//! Register the metadata functions and classes.

use crate::planner_changes::_helpers::metadata;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the metadata functions and classes.
#[inline]
pub fn register_metadata(module: &Bound<'_, PyModule>) -> PyResult<()> {
    metadata::register(module)
}
