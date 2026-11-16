//! Project static relation data for Python checks.

use crate::planner_entries::_helpers::plans::command_plans::StaticPlanParts;
use crate::planner_entries::_helpers::plans::command_plans::base_plan_output;
use pyo3::prelude::{Bound, PyAny, PyResult};

/// Project static relation data for Python checks.
pub fn build_relation_command_plan<'py>(
    project: &Bound<'py, PyAny>,
    scope: &Bound<'py, PyAny>,
    relations: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    base_plan_output(project, scope, relations, StaticPlanParts::default())
}
