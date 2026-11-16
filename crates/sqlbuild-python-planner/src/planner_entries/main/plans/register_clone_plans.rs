//! Register the clone plans functions and classes.

use crate::planner_entries::_helpers::plans::clone_plans;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the clone plans functions and classes.
#[inline]
pub fn register_clone_plans(module: &Bound<'_, PyModule>) -> PyResult<()> {
    clone_plans::register(module)
}
