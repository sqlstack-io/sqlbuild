//! Write through the interpreter's live console, respecting redirection and surfacing delivery errors.

use crate::native_host::_helpers::host;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// Write through the interpreter's live console, respecting redirection and surfacing delivery errors.
#[inline]
pub fn write_console(py: Python<'_>, stream: &str, text: &str) -> PyResult<()> {
    host::write_console(py, stream, text)
}
