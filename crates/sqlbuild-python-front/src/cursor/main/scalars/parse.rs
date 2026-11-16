//! One cursor scalar, preserving date-ness and timestamp offsets.

use crate::cursor::_helpers::scalars;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// One cursor scalar, preserving date-ness and timestamp offsets.
#[inline]
pub fn parse<'py>(
    raw: &Bound<'py, PyAny>,
    cursor_type: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    scalars::parse(raw, cursor_type)
}
