//! Register the orchestration functions and classes.

use crate::planner_resolution::_helpers::references::orchestration;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the orchestration functions and classes.
#[inline]
pub fn register_orchestration(module: &Bound<'_, PyModule>) -> PyResult<()> {
    orchestration::register(module)
}
