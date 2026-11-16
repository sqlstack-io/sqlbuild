//! Register the fixture completion functions and classes.

use crate::planner_strategy::_helpers::fixture_completion;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the fixture completion functions and classes.
#[inline]
pub fn register_fixture_completion(module: &Bound<'_, PyModule>) -> PyResult<()> {
    fixture_completion::register(module)
}
