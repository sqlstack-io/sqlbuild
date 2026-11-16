//! The canonical partition containing an observed physical value.

use crate::cursor::_helpers::intervals;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// The canonical partition containing an observed physical value.
#[inline]
pub fn partition<'py>(
    value: &Bound<'py, PyAny>,
    grain: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    intervals::partition(value, grain)
}
