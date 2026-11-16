//! `lint_file_role` as a function of `module`.

use crate::rules::_helpers::lint::lint_headers::lint_file_role;

/// `lint_file_role` as a function of `module`.
pub fn lint_file_role_function<'py>(
    module: &pyo3::Bound<'py, pyo3::types::PyModule>,
) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::types::PyCFunction>> {
    pyo3::wrap_pyfunction!(lint_file_role, module)
}
