//! `build_effective_target_namespace` as a function of `module`.

use crate::compile_attachment::_helpers::target::build_effective_target_namespace;

/// `build_effective_target_namespace` as a function of `module`.
pub fn build_effective_target_namespace_function<'py>(
    module: &pyo3::Bound<'py, pyo3::types::PyModule>,
) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::types::PyCFunction>> {
    pyo3::wrap_pyfunction!(build_effective_target_namespace, module)
}
