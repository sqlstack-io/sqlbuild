//! The terminal's width when it is known: `shutil.get_terminal_size()` without a fallback width.

use crate::build_progress::_helpers::terminal;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// The terminal's width as it reports it, or `COLUMNS`; `None` when neither says.
#[inline]
pub fn known_terminal_columns(py: Python<'_>) -> PyResult<Option<usize>> {
    terminal::known_terminal_columns(py)
}
