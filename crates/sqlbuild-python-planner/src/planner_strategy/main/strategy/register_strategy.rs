//! Register the strategy functions and classes.

use crate::planner_strategy::_helpers::strategy;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the strategy functions and classes.
#[inline]
pub fn register_strategy(module: &Bound<'_, PyModule>) -> PyResult<()> {
    strategy::register(module)
}
