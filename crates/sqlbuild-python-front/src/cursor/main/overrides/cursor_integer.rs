//! An integer override as an `int`; text is read the way `int(Decimal(text))` reads it.

use crate::cursor::_helpers::overrides;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// An integer override as an `int`; text is read the way `int(Decimal(text))` reads it.
#[inline]
pub fn cursor_integer<'py>(value: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    overrides::cursor_integer(value)
}
