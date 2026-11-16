//! Raise `CursorAlgebraError` unless the ends form a valid aligned interval.

use crate::cursor::_helpers::intervals;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Raise `CursorAlgebraError` unless the ends form a valid aligned interval.
#[inline]
pub fn interval_validation(
    start: &Bound<'_, PyAny>,
    end: &Bound<'_, PyAny>,
    grain: &Bound<'_, PyAny>,
) -> PyResult<()> {
    intervals::interval_validation(start, end, grain)
}
