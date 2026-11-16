//! `resolve_scenario_check_sql` as a function of `module`.

use crate::planner_scenario::_helpers::sql::resolve_scenario_check_sql;

/// `resolve_scenario_check_sql` as a function of `module`.
pub fn resolve_scenario_check_sql_function<'py>(
    module: &pyo3::Bound<'py, pyo3::types::PyModule>,
) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::types::PyCFunction>> {
    pyo3::wrap_pyfunction!(resolve_scenario_check_sql, module)
}
