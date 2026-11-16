//! One project var value as interpolated text; structured values are rejected.

use crate::authored_values::_helpers::project_vars;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// One project var value as interpolated text; structured values are rejected.
#[inline]
pub fn project_var_text(value: &Bound<'_, PyAny>, label: &str) -> PyResult<String> {
    project_vars::project_var_text(value, label)
}
