//! `build_scenario_relation_map` as a function of `module`.

use crate::planner_scenario::_helpers::artifacts;

/// `build_scenario_relation_map` as a function of `module`.
#[inline]
pub fn build_scenario_relation_map_function<'py>(
    module: &pyo3::Bound<'py, pyo3::types::PyModule>,
) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::types::PyCFunction>> {
    artifacts::build_scenario_relation_map_function(module)
}
