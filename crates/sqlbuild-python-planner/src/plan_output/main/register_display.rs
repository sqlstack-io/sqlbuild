//! Register the display functions and classes.

use crate::plan_output::_helpers::display;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the display functions and classes.
#[inline]
pub fn register_display(module: &Bound<'_, PyModule>) -> PyResult<()> {
    display::register(module)
}
