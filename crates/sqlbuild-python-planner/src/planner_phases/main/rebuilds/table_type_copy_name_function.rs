//! `table_type_copy_name` as a function of `module`.

use crate::planner_phases::_helpers::rebuilds::retention::table_type_copy_name;

/// `table_type_copy_name` as a function of `module`.
pub fn table_type_copy_name_function<'py>(
    module: &pyo3::Bound<'py, pyo3::types::PyModule>,
) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::types::PyCFunction>> {
    pyo3::wrap_pyfunction!(table_type_copy_name, module)
}
