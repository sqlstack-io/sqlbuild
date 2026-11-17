//! `fix_verdict` as a function of `module`.

use crate::rules::_helpers::fix::fix_verdict::fix_verdict;

/// `fix_verdict` as a function of `module`.
pub fn fix_verdict_function<'py>(
    module: &pyo3::Bound<'py, pyo3::types::PyModule>,
) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::types::PyCFunction>> {
    pyo3::wrap_pyfunction!(fix_verdict, module)
}
