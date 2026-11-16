//! Register the fixture context functions and classes.

use crate::planner_strategy::_helpers::fixture_context;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the fixture context functions and classes.
#[inline]
pub fn register_fixture_context(module: &Bound<'_, PyModule>) -> PyResult<()> {
    fixture_context::register(module)
}
