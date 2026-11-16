//! Whether `stream` reports a terminal.

use crate::build_progress::_helpers::terminal;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Whether `stream` reports a terminal.
#[inline]
pub fn tty_flag(stream: &Bound<'_, PyAny>) -> PyResult<bool> {
    terminal::tty_flag(stream)
}
