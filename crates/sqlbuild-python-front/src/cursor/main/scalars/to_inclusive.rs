//! An exclusive bound moved back by one cursor unit.

use crate::cursor::_helpers::scalars;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// An exclusive bound moved back by one cursor unit.
#[inline]
pub fn to_inclusive<'py>(
    value: &Bound<'py, PyAny>,
    grain: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    scalars::to_inclusive(value, grain)
}
