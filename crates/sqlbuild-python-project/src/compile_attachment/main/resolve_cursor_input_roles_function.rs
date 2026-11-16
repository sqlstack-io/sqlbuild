//! `resolve_cursor_input_roles` as a function of `module`.

use crate::compile_attachment::_helpers::cursor_roles;

/// `resolve_cursor_input_roles` as a function of `module`.
#[inline]
pub fn resolve_cursor_input_roles_function<'py>(
    module: &pyo3::Bound<'py, pyo3::types::PyModule>,
) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::types::PyCFunction>> {
    cursor_roles::resolve_cursor_input_roles_function(module)
}
