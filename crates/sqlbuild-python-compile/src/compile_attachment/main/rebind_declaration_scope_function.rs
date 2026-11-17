//! `rebind_declaration_scope` as a function of `module`.

use crate::compile_attachment::_helpers::declaration_scope::rebind_declaration_scope;

/// `rebind_declaration_scope` as a function of `module`.
pub fn rebind_declaration_scope_function<'py>(
    module: &pyo3::Bound<'py, pyo3::types::PyModule>,
) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::types::PyCFunction>> {
    pyo3::wrap_pyfunction!(rebind_declaration_scope, module)
}
