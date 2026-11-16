//! `warnings` followed by the future-end and automatic-start warnings `bounds` carries.

use crate::planner_resolution::_helpers::cursor::cursor_warnings;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// `warnings` followed by the future-end and automatic-start warnings `bounds` carries.
#[inline]
pub fn with_cursor_safety_warnings<'py>(
    warnings: &Bound<'py, PyAny>,
    model_name: &Bound<'py, PyAny>,
    bounds: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    cursor_warnings::with_cursor_safety_warnings(warnings, model_name, bounds)
}
