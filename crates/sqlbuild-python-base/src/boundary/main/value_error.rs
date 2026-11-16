//! The same as `compiler_error`.

use crate::boundary::_helpers::panics;
use pyo3::PyErr;

/// The same as `compiler_error`.
#[inline]
pub fn value_error(error: impl std::fmt::Display) -> PyErr {
    panics::value_error(error)
}
