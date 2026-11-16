//! `build_scenario_hash_index` as a function of `module`.

use crate::planner_scenario::_helpers::artifacts::build_scenario_hash_index;

/// `build_scenario_hash_index` as a function of `module`.
pub fn build_scenario_hash_index_function<'py>(
    module: &pyo3::Bound<'py, pyo3::types::PyModule>,
) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::types::PyCFunction>> {
    pyo3::wrap_pyfunction!(build_scenario_hash_index, module)
}
