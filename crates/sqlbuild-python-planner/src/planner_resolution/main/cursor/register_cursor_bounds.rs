//! Register the cursor bounds functions and classes.

use crate::planner_resolution::_helpers::cursor::cursor_bounds;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the cursor bounds functions and classes.
#[inline]
pub fn register_cursor_bounds(module: &Bound<'_, PyModule>) -> PyResult<()> {
    cursor_bounds::register(module)
}
