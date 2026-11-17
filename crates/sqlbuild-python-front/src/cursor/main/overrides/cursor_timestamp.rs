//! A timestamp override as a `datetime`; dates start at midnight and text is ISO 8601.

use crate::cursor::_helpers::overrides;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// A timestamp override as a `datetime`; dates start at midnight and text is ISO 8601.
#[inline]
pub fn cursor_timestamp<'py>(value: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    overrides::cursor_timestamp(value)
}
