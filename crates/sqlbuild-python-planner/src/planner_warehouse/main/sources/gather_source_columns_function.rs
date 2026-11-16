//! `gather_source_columns` as a function of `module`.

use crate::planner_warehouse::_helpers::sources::source_columns;

/// `gather_source_columns` as a function of `module`.
#[inline]
pub fn gather_source_columns_function<'py>(
    module: &pyo3::Bound<'py, pyo3::types::PyModule>,
) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::types::PyCFunction>> {
    source_columns::gather_source_columns_function(module)
}
