//! Register the cursor warnings functions and classes.

use crate::planner_resolution::_helpers::cursor::cursor_warnings;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the cursor warnings functions and classes.
#[inline]
pub fn register_cursor_warnings(module: &Bound<'_, PyModule>) -> PyResult<()> {
    cursor_warnings::register(module)
}
