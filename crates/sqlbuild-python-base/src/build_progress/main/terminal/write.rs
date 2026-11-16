//! Write.

use crate::build_progress::_helpers::terminal;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Write.
#[inline]
pub fn write(stream: &Bound<'_, PyAny>, text: &str) -> PyResult<()> {
    terminal::write(stream, text)
}
