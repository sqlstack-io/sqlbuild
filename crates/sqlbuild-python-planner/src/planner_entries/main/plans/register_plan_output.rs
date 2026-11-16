//! Register the plan output functions and classes.

use crate::planner_entries::_helpers::plans::plan_output;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the plan output functions and classes.
#[inline]
pub fn register_plan_output(module: &Bound<'_, PyModule>) -> PyResult<()> {
    plan_output::register(module)
}
