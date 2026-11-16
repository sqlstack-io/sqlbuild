//! `build_compile_target_context` as a function of `module`.

use crate::compile_attachment::_helpers::target::build_compile_target_context;

/// `build_compile_target_context` as a function of `module`.
pub fn build_compile_target_context_function<'py>(
    module: &pyo3::Bound<'py, pyo3::types::PyModule>,
) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::types::PyCFunction>> {
    pyo3::wrap_pyfunction!(build_compile_target_context, module)
}
