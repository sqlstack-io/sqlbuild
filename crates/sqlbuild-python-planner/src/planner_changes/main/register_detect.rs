//! Register the detect functions and classes.

use crate::planner_changes::_helpers::detect;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the detect functions and classes.
#[inline]
pub fn register_detect(module: &Bound<'_, PyModule>) -> PyResult<()> {
    detect::register(module)
}
