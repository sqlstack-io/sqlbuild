//! Flush.

use crate::build_progress::_helpers::terminal;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Flush.
#[inline]
pub fn flush(stream: &Bound<'_, PyAny>) -> PyResult<()> {
    terminal::flush(stream)
}
