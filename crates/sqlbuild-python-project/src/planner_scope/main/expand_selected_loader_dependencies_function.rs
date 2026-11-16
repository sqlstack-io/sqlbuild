//! `expand_selected_loader_dependencies` as a function of `module`.

use crate::planner_scope::_helpers::loaders;

/// `expand_selected_loader_dependencies` as a function of `module`.
#[inline]
pub fn expand_selected_loader_dependencies_function<'py>(
    module: &pyo3::Bound<'py, pyo3::types::PyModule>,
) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::types::PyCFunction>> {
    loaders::expand_selected_loader_dependencies_function(module)
}
