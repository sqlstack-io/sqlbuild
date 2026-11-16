//! Release.

use crate::build_progress::_helpers::terminal;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Release.
#[inline]
pub fn release(lines: &Bound<'_, PyAny>, owner: &Bound<'_, PyAny>) -> PyResult<()> {
    terminal::release(lines, owner)
}
