//! `declaration_rows` as a Python function.

use crate::compiled_views::main::project::declaration_rows::declaration_rows;

/// `declaration_rows` as a Python function.
pub fn declaration_rows_function(
    py: pyo3::Python<'_>,
) -> pyo3::PyResult<pyo3::Bound<'_, pyo3::types::PyCFunction>> {
    pyo3::wrap_pyfunction!(declaration_rows, py)
}
