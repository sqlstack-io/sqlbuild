//! Register the renames functions and classes.

use crate::planner_changes::_helpers::renames;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the renames functions and classes.
#[inline]
pub fn register_renames(module: &Bound<'_, PyModule>) -> PyResult<()> {
    renames::register(module)
}
