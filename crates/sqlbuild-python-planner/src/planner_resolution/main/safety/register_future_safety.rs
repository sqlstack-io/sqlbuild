//! Register the future safety functions and classes.

use crate::planner_resolution::_helpers::safety::future_safety;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the future safety functions and classes.
#[inline]
pub fn register_future_safety(module: &Bound<'_, PyModule>) -> PyResult<()> {
    future_safety::register(module)
}
