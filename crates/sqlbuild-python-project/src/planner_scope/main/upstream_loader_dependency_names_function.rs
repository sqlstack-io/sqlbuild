//! `upstream_loader_dependency_names` as a function of `module`.

use crate::planner_scope::_helpers::loaders;

/// `upstream_loader_dependency_names` as a function of `module`.
#[inline]
pub fn upstream_loader_dependency_names_function<'py>(
    module: &pyo3::Bound<'py, pyo3::types::PyModule>,
) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::types::PyCFunction>> {
    loaders::upstream_loader_dependency_names_function(module)
}
