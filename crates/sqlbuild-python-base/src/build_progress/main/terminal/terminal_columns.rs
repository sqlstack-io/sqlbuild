//! The current terminal width, `shutil.get_terminal_size(fallback=(120, 24)).columns`.

use crate::build_progress::_helpers::terminal;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// The current terminal width, `shutil.get_terminal_size(fallback=(120, 24)).columns`.
#[inline]
pub fn terminal_columns(py: Python<'_>) -> PyResult<usize> {
    terminal::terminal_columns(py)
}
