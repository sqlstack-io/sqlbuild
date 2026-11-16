//! `resolve_cursor_input_roles` as a function of `module`.

use crate::compile_attachment::main::resolve_cursor_input_roles::resolve_cursor_input_roles;

/// `resolve_cursor_input_roles` as a function of `module`.
pub fn resolve_cursor_input_roles_function<'py>(
    module: &pyo3::Bound<'py, pyo3::types::PyModule>,
) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::types::PyCFunction>> {
    pyo3::wrap_pyfunction!(resolve_cursor_input_roles, module)
}
