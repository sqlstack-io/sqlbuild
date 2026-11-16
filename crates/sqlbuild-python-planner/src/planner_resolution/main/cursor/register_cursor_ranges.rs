//! Register the cursor ranges functions and classes.

use crate::planner_resolution::_helpers::cursor::cursor_ranges;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the cursor ranges functions and classes.
#[inline]
pub fn register_cursor_ranges(module: &Bound<'_, PyModule>) -> PyResult<()> {
    cursor_ranges::register(module)
}
