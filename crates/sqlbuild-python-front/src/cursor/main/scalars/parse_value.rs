//! One cursor value parsed from its boundary representation, keeping date-ness and offsets.

use crate::cursor::_helpers::scalars;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// One cursor value parsed from its boundary representation, keeping date-ness and offsets.
#[inline]
pub fn parse_value<'py>(
    raw: &Bound<'py, PyAny>,
    cursor_type: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    scalars::parse_value(raw, cursor_type)
}
