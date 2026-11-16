//! Rule out the running delta compile: returns the error that unwinds to its fallback.

use crate::compile_context::_helpers::delta;

/// Rule out the running delta compile: returns the error that unwinds to its fallback.
#[inline]
pub fn delta_unsupported(reason: &str) -> pyo3::PyErr {
    delta::delta_unsupported(reason)
}
