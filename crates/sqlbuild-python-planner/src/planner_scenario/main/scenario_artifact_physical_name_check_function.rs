//! `is_scenario_artifact_physical_name` as a function of `module`.

use crate::planner_scenario::main::is_scenario_artifact_physical_name::is_scenario_artifact_physical_name;

/// `is_scenario_artifact_physical_name` as a function of `module`.
pub fn scenario_artifact_physical_name_check_function<'py>(
    module: &pyo3::Bound<'py, pyo3::types::PyModule>,
) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::types::PyCFunction>> {
    pyo3::wrap_pyfunction!(is_scenario_artifact_physical_name, module)
}
