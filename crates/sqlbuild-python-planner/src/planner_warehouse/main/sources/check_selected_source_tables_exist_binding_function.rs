//! `check_selected_source_tables_exist_binding` as a function of `module`.

use crate::planner_warehouse::_helpers::sources::source_tables::check_selected_source_tables_exist_binding;

/// `check_selected_source_tables_exist_binding` as a function of `module`.
pub fn check_selected_source_tables_exist_binding_function<'py>(
    module: &pyo3::Bound<'py, pyo3::types::PyModule>,
) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::types::PyCFunction>> {
    pyo3::wrap_pyfunction!(check_selected_source_tables_exist_binding, module)
}
