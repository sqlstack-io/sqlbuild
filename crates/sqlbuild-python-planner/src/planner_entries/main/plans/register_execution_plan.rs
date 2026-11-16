//! Add the execution plan entry point.

use crate::planner_entries::_helpers::plans::execution_plan;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Add the execution plan entry point.
#[inline]
pub fn register_execution_plan(module: &Bound<'_, PyModule>) -> PyResult<()> {
    execution_plan::register(module)
}
