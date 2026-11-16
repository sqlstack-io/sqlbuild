//! Register the cursor overrides functions and classes.

use crate::planner_resolution::_helpers::cursor::cursor_overrides;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the cursor overrides functions and classes.
#[inline]
pub fn register_cursor_overrides(module: &Bound<'_, PyModule>) -> PyResult<()> {
    cursor_overrides::register(module)
}
