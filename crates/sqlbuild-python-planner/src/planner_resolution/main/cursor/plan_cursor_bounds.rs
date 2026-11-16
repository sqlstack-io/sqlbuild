//! Compute cursor bounds for inclusion on the plan entry.

use crate::planner_resolution::_helpers::cursor::cursor_ranges;
use crate::planner_resolution::models::CursorWindowInputs;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Compute cursor bounds for inclusion on the plan entry.
#[inline]
pub fn plan_cursor_bounds<'py>(
    model: &Bound<'py, PyAny>,
    inputs: &CursorWindowInputs<'py>,
) -> PyResult<Bound<'py, PyAny>> {
    cursor_ranges::plan_cursor_bounds(model, inputs)
}
