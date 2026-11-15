//! A panic message as `NativeCompilerError`, anything else as `ValueError`.

use crate::boundary::_helpers::panics;
use pyo3::PyErr;

/// A panic message as `NativeCompilerError`, anything else as `ValueError`.
#[inline]
pub fn compiler_error(error: impl std::fmt::Display) -> PyErr {
    panics::compiler_error(error)
}
