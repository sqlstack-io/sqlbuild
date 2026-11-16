//! Register the planning functions and classes.

use crate::freshness::_helpers::planning;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the planning functions and classes.
#[inline]
pub fn register_planning(module: &Bound<'_, PyModule>) -> PyResult<()> {
    planning::register(module)
}
