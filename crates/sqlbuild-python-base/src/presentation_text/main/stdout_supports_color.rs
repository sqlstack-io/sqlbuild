//! Whether `sys.stdout` is a terminal and `NO_COLOR` is unset.

use crate::presentation_text::_helpers::terminal;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// Whether `sys.stdout` is a terminal and `NO_COLOR` is unset.
#[inline]
pub fn stdout_supports_color(py: Python<'_>) -> PyResult<bool> {
    terminal::stdout_supports_color(py)
}
