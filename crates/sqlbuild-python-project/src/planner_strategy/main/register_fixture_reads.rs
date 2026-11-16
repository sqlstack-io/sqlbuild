//! Register the fixture reads functions and classes.

use crate::planner_strategy::_helpers::fixture_reads;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the fixture reads functions and classes.
#[inline]
pub fn register_fixture_reads(module: &Bound<'_, PyModule>) -> PyResult<()> {
    fixture_reads::register(module)
}
