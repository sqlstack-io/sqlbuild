//! `build_sql_expansion_context` as a function of `module`.

use crate::compile_attachment::_helpers::sql_expansion_context::build_sql_expansion_context;

/// `build_sql_expansion_context` as a function of `module`.
pub fn build_sql_expansion_context_function<'py>(
    module: &pyo3::Bound<'py, pyo3::types::PyModule>,
) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::types::PyCFunction>> {
    pyo3::wrap_pyfunction!(build_sql_expansion_context, module)
}
