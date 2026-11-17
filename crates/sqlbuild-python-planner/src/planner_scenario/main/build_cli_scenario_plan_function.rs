//! `build_cli_scenario_plan` as a function of `module`.

use crate::planner_scenario::_helpers::cli::build_cli_scenario_plan;

/// `build_cli_scenario_plan` as a function of `module`.
pub fn build_cli_scenario_plan_function<'py>(
    module: &pyo3::Bound<'py, pyo3::types::PyModule>,
) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::types::PyCFunction>> {
    pyo3::wrap_pyfunction!(build_cli_scenario_plan, module)
}
