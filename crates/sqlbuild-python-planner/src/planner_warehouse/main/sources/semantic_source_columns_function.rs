//! `semantic_source_columns` as a function of `module`.

use crate::planner_warehouse::_helpers::sources::semantic_sources::semantic_source_columns;

/// `semantic_source_columns` as a function of `module`.
pub fn semantic_source_columns_function<'py>(
    module: &pyo3::Bound<'py, pyo3::types::PyModule>,
) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::types::PyCFunction>> {
    pyo3::wrap_pyfunction!(semantic_source_columns, module)
}
