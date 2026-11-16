//! `resolve_scenario_check_sql` as a function of `module`.

use crate::planner_scenario::_helpers::sql;

/// `resolve_scenario_check_sql` as a function of `module`.
#[inline]
pub fn resolve_scenario_check_sql_function<'py>(
    module: &pyo3::Bound<'py, pyo3::types::PyModule>,
) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::types::PyCFunction>> {
    sql::resolve_scenario_check_sql_function(module)
}
