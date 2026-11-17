//! `build_planner_source_freshness_result_of` as a function of `module`.

use crate::planner_warehouse::_helpers::sources::source_freshness::build_planner_source_freshness_result_of;

/// `build_planner_source_freshness_result_of` as a function of `module`.
pub fn build_planner_source_freshness_result_of_function<'py>(
    module: &pyo3::Bound<'py, pyo3::types::PyModule>,
) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::types::PyCFunction>> {
    pyo3::wrap_pyfunction!(build_planner_source_freshness_result_of, module)
}
