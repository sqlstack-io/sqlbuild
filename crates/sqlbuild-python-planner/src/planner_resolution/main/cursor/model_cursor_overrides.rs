//! Resolve typed cursor overrides to generic start/end for one model.

use crate::planner_resolution::_helpers::cursor::cursor_ranges;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Resolve typed cursor overrides to generic start/end for one model.
#[inline]
pub fn model_cursor_overrides<'py>(
    model: &Bound<'py, PyAny>,
    cursor_overrides: &Bound<'py, PyAny>,
    (start, end): (Bound<'py, PyAny>, Bound<'py, PyAny>),
) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>)> {
    cursor_ranges::model_cursor_overrides(model, cursor_overrides, (start, end))
}
