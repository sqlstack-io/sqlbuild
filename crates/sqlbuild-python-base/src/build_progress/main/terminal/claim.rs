//! Claim or release the live line for `owner`.

use crate::build_progress::_helpers::terminal;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Claim or release the live line for `owner`.
#[inline]
pub fn claim(
    lines: &Bound<'_, PyAny>,
    stream: &Bound<'_, PyAny>,
    owner: &Bound<'_, PyAny>,
) -> PyResult<()> {
    terminal::claim(lines, stream, owner)
}
