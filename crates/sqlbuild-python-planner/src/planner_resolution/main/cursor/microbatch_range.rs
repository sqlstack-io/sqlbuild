//! Compute the real overall cursor range for microbatch batch splitting.

use crate::planner_resolution::_helpers::cursor::cursor_ranges;
use crate::planner_resolution::models::CursorWindowInputs;
use pyo3::prelude::{Bound, PyAny, PyResult};

/// Compute the real overall cursor range for microbatch batch splitting.
#[inline]
pub fn microbatch_range<'py>(
    model: &Bound<'py, PyAny>,
    inputs: &CursorWindowInputs<'py>,
) -> PyResult<Bound<'py, PyAny>> {
    cursor_ranges::microbatch_range(model, inputs)
}
