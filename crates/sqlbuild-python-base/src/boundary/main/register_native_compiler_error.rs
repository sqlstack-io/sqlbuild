//! Add `NativeCompilerError` to the extension module.

use crate::boundary::_helpers::panics;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Add `NativeCompilerError` to the extension module.
#[inline]
pub fn register_native_compiler_error(module: &Bound<'_, PyModule>) -> PyResult<()> {
    panics::register_native_compiler_error(module)
}
