//! `build_planner_source_freshness_result_of` as a function of `module`.

use crate::planner_warehouse::_helpers::sources::source_freshness;

/// `build_planner_source_freshness_result_of` as a function of `module`.
#[inline]
pub fn build_planner_source_freshness_result_of_function<'py>(
    module: &pyo3::Bound<'py, pyo3::types::PyModule>,
) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::types::PyCFunction>> {
    source_freshness::build_planner_source_freshness_result_of_function(module)
}
