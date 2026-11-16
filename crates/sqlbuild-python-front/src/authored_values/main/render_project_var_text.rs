//! One project var value as interpolated text; structured values are rejected.

use crate::authored_values::main::project_var_text::project_var_text;
use pyo3::prelude::{Bound, PyAny, PyResult};
use pyo3::pyfunction;

/// One project var value as interpolated text; structured values are rejected.
#[pyfunction]
#[pyo3(signature = (*, value, label))]
pub fn render_project_var_text(value: &Bound<'_, PyAny>, label: &str) -> PyResult<String> {
    project_var_text(value, label)
}
