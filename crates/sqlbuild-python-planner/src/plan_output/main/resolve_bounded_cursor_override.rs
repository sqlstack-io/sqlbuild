//! A fully bounded inclusive operator override as an executable interval.

use crate::plan_output::_helpers::display;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// A fully bounded inclusive operator override as an executable interval.
#[inline]
pub fn resolve_bounded_cursor_override<'py>(
    start_cursor_override: &Bound<'py, PyAny>,
    end_cursor_override: &Bound<'py, PyAny>,
    cursor_type: &Bound<'py, PyAny>,
    cursor_grain: &Bound<'py, PyAny>,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    display::resolve_bounded_cursor_override(
        start_cursor_override,
        end_cursor_override,
        cursor_type,
        cursor_grain,
    )
}
