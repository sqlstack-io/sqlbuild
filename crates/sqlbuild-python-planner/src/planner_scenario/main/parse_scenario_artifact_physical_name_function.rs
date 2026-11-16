//! `parse_scenario_artifact_physical_name` as a function of `module`.

use crate::planner_scenario::_helpers::artifacts::parse_scenario_artifact_physical_name;

/// `parse_scenario_artifact_physical_name` as a function of `module`.
pub fn parse_scenario_artifact_physical_name_function<'py>(
    module: &pyo3::Bound<'py, pyo3::types::PyModule>,
) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::types::PyCFunction>> {
    pyo3::wrap_pyfunction!(parse_scenario_artifact_physical_name, module)
}
