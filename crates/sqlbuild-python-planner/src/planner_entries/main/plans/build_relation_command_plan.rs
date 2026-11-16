//! Project static relation data for Python checks.

use crate::planner_entries::_helpers::plans::command_plans;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Project static relation data for Python checks.
#[inline]
pub fn build_relation_command_plan<'py>(
    project: &Bound<'py, PyAny>,
    scope: &Bound<'py, PyAny>,
    relations: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    command_plans::build_relation_command_plan(project, scope, relations)
}
