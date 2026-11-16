//! `plan_table_types` as a function of `module`.

use crate::planner_phases::_helpers::rebuilds::retention::plan_table_types;

/// `plan_table_types` as a function of `module`.
pub fn plan_table_types_function<'py>(
    module: &pyo3::Bound<'py, pyo3::types::PyModule>,
) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::types::PyCFunction>> {
    pyo3::wrap_pyfunction!(plan_table_types, module)
}
