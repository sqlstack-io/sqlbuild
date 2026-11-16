//! Register the maximum safety functions and classes.

use crate::planner_resolution::_helpers::safety::maximum_safety;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the maximum safety functions and classes.
#[inline]
pub fn register_maximum_safety(module: &Bound<'_, PyModule>) -> PyResult<()> {
    maximum_safety::register(module)
}
