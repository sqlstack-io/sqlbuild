//! A cursor bound for display, collapsing a whole-day midnight timestamp to a date.

use crate::plan_output::_helpers::display;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// A cursor bound for display, collapsing a whole-day midnight timestamp to a date.
#[inline]
pub fn cursor_bound_display(
    value: &Bound<'_, PyAny>,
    cursor_type: &Bound<'_, PyAny>,
    cursor_grain: &Bound<'_, PyAny>,
) -> PyResult<String> {
    display::cursor_bound_display(value, cursor_type, cursor_grain)
}
