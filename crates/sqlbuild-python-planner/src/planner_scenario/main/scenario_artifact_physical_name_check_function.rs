//! `is_scenario_artifact_physical_name` as a function of `module`.

use crate::planner_scenario::_helpers::artifacts;

/// `is_scenario_artifact_physical_name` as a function of `module`.
#[inline]
pub fn scenario_artifact_physical_name_check_function<'py>(
    module: &pyo3::Bound<'py, pyo3::types::PyModule>,
) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::types::PyCFunction>> {
    artifacts::scenario_artifact_physical_name_check_function(module)
}
