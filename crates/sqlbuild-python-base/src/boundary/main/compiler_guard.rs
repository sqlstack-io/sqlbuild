//! Run `operation`, turning an unwinding panic into `NativeCompilerError`.

use crate::boundary::_helpers::panics;

/// Run `operation`, turning an unwinding panic into `NativeCompilerError`.
#[inline]
pub fn compiler_guard<T>(operation: impl FnOnce() -> pyo3::PyResult<T>) -> pyo3::PyResult<T> {
    panics::compiler_guard(operation)
}
