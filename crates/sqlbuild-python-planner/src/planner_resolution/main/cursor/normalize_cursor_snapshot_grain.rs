//! Normalize cursor snapshot grain.

use crate::planner_resolution::_helpers::cursor::cursor_windows;
use pyo3::prelude::{Bound, PyAny, PyResult};

/// Normalize cursor snapshot grain.
#[inline]
pub fn normalize_cursor_snapshot_grain<'py>(
    cursor_snapshot: &Bound<'py, PyAny>,
    cursor_type: &Bound<'py, PyAny>,
    effective_grain: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    cursor_windows::normalize_cursor_snapshot_grain(cursor_snapshot, cursor_type, effective_grain)
}
