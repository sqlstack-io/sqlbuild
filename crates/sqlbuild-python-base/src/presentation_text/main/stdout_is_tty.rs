//! Whether `sys.stdout` is a terminal.

use crate::presentation_text::_helpers::terminal;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// Whether `sys.stdout` is a terminal.
#[inline]
pub fn stdout_is_tty(py: Python<'_>) -> PyResult<bool> {
    terminal::stdout_is_tty(py)
}
