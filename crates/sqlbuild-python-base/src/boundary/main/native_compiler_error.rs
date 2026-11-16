//! The named compiler error for an internal native failure.

use crate::boundary::_helpers::panics;
use pyo3::PyErr;

/// The named compiler error for an internal native failure.
#[inline]
pub fn native_compiler_error(message: impl Into<String>) -> PyErr {
    panics::native_compiler_error(message)
}
