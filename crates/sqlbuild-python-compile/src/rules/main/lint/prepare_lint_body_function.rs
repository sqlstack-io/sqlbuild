//! `prepare_lint_body` as a function of `module`.

use crate::rules::_helpers::lint::lint_preparation::prepare_lint_body;

/// `prepare_lint_body` as a function of `module`.
pub fn prepare_lint_body_function<'py>(
    module: &pyo3::Bound<'py, pyo3::types::PyModule>,
) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::types::PyCFunction>> {
    pyo3::wrap_pyfunction!(prepare_lint_body, module)
}
