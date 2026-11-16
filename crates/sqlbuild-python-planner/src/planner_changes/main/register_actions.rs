//! Register the actions functions and classes.

use crate::planner_changes::_helpers::actions;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the actions functions and classes.
#[inline]
pub fn register_actions(module: &Bound<'_, PyModule>) -> PyResult<()> {
    actions::register(module)
}
