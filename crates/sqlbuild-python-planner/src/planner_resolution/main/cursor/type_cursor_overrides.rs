//! Untyped cursor overrides typed by the cursor type of the selected incremental models.

use crate::planner_resolution::_helpers::cursor::cursor_overrides;
use pyo3::prelude::{Bound, PyAny, PyResult};

/// Untyped cursor overrides typed by the cursor type of the selected incremental models.
#[inline]
pub fn type_cursor_overrides<'py>(
    overrides: &Bound<'py, PyAny>,
    selected: &[Bound<'py, PyAny>],
) -> PyResult<Bound<'py, PyAny>> {
    cursor_overrides::type_cursor_overrides(overrides, selected)
}
