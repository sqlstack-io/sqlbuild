//! The last cursor value included by an exclusive end bound.

use crate::plan_output::_helpers::display;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// The last cursor value included by an exclusive end bound.
#[inline]
pub fn inclusive_cursor_end(
    end: &Bound<'_, PyAny>,
    cursor_type: &Bound<'_, PyAny>,
    cursor_grain: &Bound<'_, PyAny>,
) -> PyResult<String> {
    display::inclusive_cursor_end(end, cursor_type, cursor_grain)
}
