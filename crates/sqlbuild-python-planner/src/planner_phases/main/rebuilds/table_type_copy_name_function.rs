//! `table_type_copy_name` as a function of `module`.

use crate::planner_phases::_helpers::rebuilds::retention;

/// `table_type_copy_name` as a function of `module`.
#[inline]
pub fn table_type_copy_name_function<'py>(
    module: &pyo3::Bound<'py, pyo3::types::PyModule>,
) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::types::PyCFunction>> {
    retention::table_type_copy_name_function(module)
}
