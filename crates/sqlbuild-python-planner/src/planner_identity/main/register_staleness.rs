//! Register the staleness functions and classes.

use crate::planner_identity::_helpers::staleness;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the staleness functions and classes.
#[inline]
pub fn register_staleness(module: &Bound<'_, PyModule>) -> PyResult<()> {
    staleness::register(module)
}
