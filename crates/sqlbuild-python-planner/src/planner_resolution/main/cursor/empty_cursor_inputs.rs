//! Empty inputs that leave a planner-owned window empty, and whether others have rows.

use crate::planner_resolution::_helpers::cursor::cursor_ranges;
use crate::planner_resolution::models::CursorWindowInputs;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Empty inputs that leave a planner-owned window empty, and whether others have rows.
#[inline]
pub fn empty_cursor_inputs<'py>(
    model: &Bound<'py, PyAny>,
    inputs: &CursorWindowInputs<'py>,
) -> PyResult<(Bound<'py, PyAny>, bool)> {
    cursor_ranges::empty_cursor_inputs(model, inputs)
}
