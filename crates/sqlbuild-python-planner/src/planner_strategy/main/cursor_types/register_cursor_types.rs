//! Register the cursor types functions and classes.

use crate::planner_strategy::_helpers::cursor_types;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the cursor types functions and classes.
#[inline]
pub fn register_cursor_types(module: &Bound<'_, PyModule>) -> PyResult<()> {
    cursor_types::register(module)
}
