//! `build_scenario_execution_plan` as a function of `module`.

use crate::planner_scenario::_helpers::execution::build_scenario_execution_plan;

/// `build_scenario_execution_plan` as a function of `module`.
pub fn build_scenario_execution_plan_function<'py>(
    module: &pyo3::Bound<'py, pyo3::types::PyModule>,
) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::types::PyCFunction>> {
    pyo3::wrap_pyfunction!(build_scenario_execution_plan, module)
}
