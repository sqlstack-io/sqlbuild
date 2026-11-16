//! `declaration_rows` as a Python function.

use crate::compiled_views::_helpers::records;

/// `declaration_rows` as a Python function.
#[inline]
pub fn declaration_rows_function(
    py: pyo3::Python<'_>,
) -> pyo3::PyResult<pyo3::Bound<'_, pyo3::types::PyCFunction>> {
    records::declaration_rows_function(py)
}
