//! Register the cursor windows functions and classes.

use crate::planner_resolution::_helpers::cursor::cursor_windows;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the cursor windows functions and classes.
#[inline]
pub fn register_cursor_windows(module: &Bound<'_, PyModule>) -> PyResult<()> {
    cursor_windows::register(module)
}
