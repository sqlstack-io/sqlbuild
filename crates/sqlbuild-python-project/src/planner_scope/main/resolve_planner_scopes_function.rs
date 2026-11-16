//! `resolve_planner_scopes` as a function of `module`.

use crate::planner_scope::main::resolve_planner_scopes::resolve_planner_scopes;

/// `resolve_planner_scopes` as a function of `module`.
pub fn resolve_planner_scopes_function<'py>(
    module: &pyo3::Bound<'py, pyo3::types::PyModule>,
) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::types::PyCFunction>> {
    pyo3::wrap_pyfunction!(resolve_planner_scopes, module)
}
