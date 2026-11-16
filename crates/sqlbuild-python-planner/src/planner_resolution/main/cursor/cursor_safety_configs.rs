//! The future-end and automatic-start policies effective for one model.

use crate::planner_resolution::_helpers::cursor::cursor_ranges;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// The future-end and automatic-start policies effective for one model.
#[inline]
pub fn cursor_safety_configs<'py>(
    model: &Bound<'py, PyAny>,
    context: &Bound<'py, PyAny>,
) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>)> {
    cursor_ranges::cursor_safety_configs(model, context)
}
