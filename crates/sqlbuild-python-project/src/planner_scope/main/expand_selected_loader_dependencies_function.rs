//! `expand_selected_loader_dependencies` as a function of `module`.

use crate::planner_scope::_helpers::loaders::expand_selected_loader_dependencies;

/// `expand_selected_loader_dependencies` as a function of `module`.
pub fn expand_selected_loader_dependencies_function<'py>(
    module: &pyo3::Bound<'py, pyo3::types::PyModule>,
) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::types::PyCFunction>> {
    pyo3::wrap_pyfunction!(expand_selected_loader_dependencies, module)
}
