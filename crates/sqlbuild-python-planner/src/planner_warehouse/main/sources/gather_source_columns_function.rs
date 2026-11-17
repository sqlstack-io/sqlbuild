//! `gather_source_columns` as a function of `module`.

use crate::planner_warehouse::main::sources::gather_source_columns::gather_source_columns;

/// `gather_source_columns` as a function of `module`.
pub fn gather_source_columns_function<'py>(
    module: &pyo3::Bound<'py, pyo3::types::PyModule>,
) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::types::PyCFunction>> {
    pyo3::wrap_pyfunction!(gather_source_columns, module)
}
