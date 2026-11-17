//! `build_direct_source_freshness_propagation_result` as a Python function.

use crate::freshness::_helpers::planning::build_direct_source_freshness_propagation_result;

/// `build_direct_source_freshness_propagation_result` as a Python function.
pub fn build_direct_source_freshness_propagation_result_function(
    py: pyo3::Python<'_>,
) -> pyo3::PyResult<pyo3::Bound<'_, pyo3::types::PyCFunction>> {
    pyo3::wrap_pyfunction!(build_direct_source_freshness_propagation_result, py)
}
