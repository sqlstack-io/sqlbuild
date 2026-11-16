//! `build_scenario_artifact_name` as a function of `module`.

use crate::planner_scenario::_helpers::artifacts;

/// `build_scenario_artifact_name` as a function of `module`.
#[inline]
pub fn build_scenario_artifact_name_function<'py>(
    module: &pyo3::Bound<'py, pyo3::types::PyModule>,
) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::types::PyCFunction>> {
    artifacts::build_scenario_artifact_name_function(module)
}
