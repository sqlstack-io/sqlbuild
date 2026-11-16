//! `CursorAlgebraError(message)`.

use crate::cursor::_helpers::conversion;
use pyo3::prelude::PyErr;
use pyo3::prelude::Python;

/// `CursorAlgebraError(message)`.
#[inline]
pub fn algebra_error(py: Python<'_>, message: &str) -> PyErr {
    conversion::algebra_error(py, message)
}
