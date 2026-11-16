//! Register the display plan functions and classes.

use crate::planner_entries::_helpers::plans::display_plan;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the display plan functions and classes.
#[inline]
pub fn register_display_plan(module: &Bound<'_, PyModule>) -> PyResult<()> {
    display_plan::register(module)
}
