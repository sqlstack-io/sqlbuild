//! `record_template_reads` as a Python function.

use crate::model_config::_helpers::config::template_data::record_template_reads;

/// `record_template_reads` as a Python function.
pub fn record_template_reads_function(
    py: pyo3::Python<'_>,
) -> pyo3::PyResult<pyo3::Bound<'_, pyo3::types::PyCFunction>> {
    pyo3::wrap_pyfunction!(record_template_reads, py)
}
