//! `record_template_reads` as a Python function.

use crate::model_config::_helpers::config::template_data;

/// `record_template_reads` as a Python function.
#[inline]
pub fn record_template_reads_function(
    py: pyo3::Python<'_>,
) -> pyo3::PyResult<pyo3::Bound<'_, pyo3::types::PyCFunction>> {
    template_data::record_template_reads_function(py)
}
