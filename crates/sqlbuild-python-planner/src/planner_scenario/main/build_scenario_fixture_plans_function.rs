//! `build_scenario_fixture_plans` as a function of `module`.

use crate::planner_scenario::_helpers::fixtures::build_scenario_fixture_plans;

/// `build_scenario_fixture_plans` as a function of `module`.
pub fn build_scenario_fixture_plans_function<'py>(
    module: &pyo3::Bound<'py, pyo3::types::PyModule>,
) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::types::PyCFunction>> {
    pyo3::wrap_pyfunction!(build_scenario_fixture_plans, module)
}
