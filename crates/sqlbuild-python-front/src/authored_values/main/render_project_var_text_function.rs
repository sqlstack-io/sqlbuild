//! `render_project_var_text` as a Python function.

use crate::authored_values::main::render_project_var_text::render_project_var_text;

/// `render_project_var_text` as a Python function.
pub fn render_project_var_text_function(
    py: pyo3::Python<'_>,
) -> pyo3::PyResult<pyo3::Bound<'_, pyo3::types::PyCFunction>> {
    pyo3::wrap_pyfunction!(render_project_var_text, py)
}
