//! Flush the interpreter's live console `stream` so text written before a child process appears first.

use crate::native_host::_helpers::host;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// Flush the interpreter's live console `stream` so text written before a child process appears first.
#[inline]
pub fn flush_console(py: Python<'_>, stream: &str) -> PyResult<()> {
    host::flush_console(py, stream)
}
