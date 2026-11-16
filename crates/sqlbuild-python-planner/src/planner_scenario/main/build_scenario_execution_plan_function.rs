//! `build_scenario_execution_plan` as a function of `module`.

use crate::planner_scenario::_helpers::execution;

/// `build_scenario_execution_plan` as a function of `module`.
#[inline]
pub fn build_scenario_execution_plan_function<'py>(
    module: &pyo3::Bound<'py, pyo3::types::PyModule>,
) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::types::PyCFunction>> {
    execution::build_scenario_execution_plan_function(module)
}
