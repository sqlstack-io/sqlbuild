//! A `CompileInputError` carrying `message`.

use crate::attachments::_helpers::cursor_intrinsics;
use pyo3::Python;
use pyo3::prelude::PyErr;
use pyo3::prelude::PyResult;

/// A `CompileInputError` carrying `message`.
#[inline]
pub fn compile_input_error(py: Python<'_>, message: String) -> PyResult<PyErr> {
    cursor_intrinsics::compile_input_error(py, message)
}
