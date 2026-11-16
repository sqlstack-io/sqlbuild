//! `render_project_var_text` as a Python function.

use crate::authored_values::_helpers::project_vars;

/// `render_project_var_text` as a Python function.
#[inline]
pub fn render_project_var_text_function(
    py: pyo3::Python<'_>,
) -> pyo3::PyResult<pyo3::Bound<'_, pyo3::types::PyCFunction>> {
    project_vars::render_project_var_text_function(py)
}
