//! `compute_scenario_hash_prefix` as a function of `module`.

use crate::planner_scenario::_helpers::artifacts::compute_scenario_hash_prefix;

/// `compute_scenario_hash_prefix` as a function of `module`.
pub fn compute_scenario_hash_prefix_function<'py>(
    module: &pyo3::Bound<'py, pyo3::types::PyModule>,
) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::types::PyCFunction>> {
    pyo3::wrap_pyfunction!(compute_scenario_hash_prefix, module)
}
