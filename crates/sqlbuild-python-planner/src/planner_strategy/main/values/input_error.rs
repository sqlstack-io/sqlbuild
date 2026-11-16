//! The native error `class(message, code=code)`.

use crate::planner_strategy::_helpers::values;
use pyo3::prelude::PyErr;
use pyo3::prelude::Python;
use sqlbuild_python_base::records::types::ErrorId;

/// The native error `class(message, code=code)`.
#[inline]
pub fn input_error(py: Python<'_>, class: ErrorId, message: &str, code: Option<&str>) -> PyErr {
    values::input_error(py, class, message, code)
}
