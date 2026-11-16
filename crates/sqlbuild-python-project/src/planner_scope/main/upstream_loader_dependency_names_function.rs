//! `upstream_loader_dependency_names` as a function of `module`.

use crate::planner_scope::main::upstream_loader_dependency_names::upstream_loader_dependency_names;

/// `upstream_loader_dependency_names` as a function of `module`.
pub fn upstream_loader_dependency_names_function<'py>(
    module: &pyo3::Bound<'py, pyo3::types::PyModule>,
) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::types::PyCFunction>> {
    pyo3::wrap_pyfunction!(upstream_loader_dependency_names, module)
}
