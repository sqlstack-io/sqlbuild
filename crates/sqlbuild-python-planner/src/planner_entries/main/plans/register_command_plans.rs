//! Register the command plans functions and classes.

use crate::planner_entries::_helpers::plans::command_plans;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the command plans functions and classes.
#[inline]
pub fn register_command_plans(module: &Bound<'_, PyModule>) -> PyResult<()> {
    command_plans::register(module)
}
