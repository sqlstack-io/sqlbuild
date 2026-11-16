//! Comparable bounds, presuming a naive side UTC when the other is aware.

use crate::plan_output::_helpers::display;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::types::PyTuple;

/// Comparable bounds, presuming a naive side UTC when the other is aware.
#[inline]
pub fn aligned_timestamp_bounds<'py>(
    start: &Bound<'py, PyAny>,
    end: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyTuple>> {
    display::aligned_timestamp_bounds(start, end)
}
