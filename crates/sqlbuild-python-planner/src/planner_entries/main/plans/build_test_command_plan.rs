//! Project selected SQL tests from canonical static state.

use crate::planner_entries::_helpers::plans::command_plans;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use pyo3::types::PyDict;

/// Project selected SQL tests from canonical static state.
#[inline]
pub fn build_test_command_plan<'py>(
    py: Python<'py>,
    kwargs: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    command_plans::build_test_command_plan(py, kwargs)
}
