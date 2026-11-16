//! `build_scenario_relation_plan_of` as a function of `module`.

use crate::planner_scenario::_helpers::relations::build_scenario_relation_plan_of;

/// `build_scenario_relation_plan_of` as a function of `module`.
pub fn build_scenario_relation_plan_of_function<'py>(
    module: &pyo3::Bound<'py, pyo3::types::PyModule>,
) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::types::PyCFunction>> {
    pyo3::wrap_pyfunction!(build_scenario_relation_plan_of, module)
}
