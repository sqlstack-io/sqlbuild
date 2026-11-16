//! Register the plan json functions and classes.

use crate::plan_output::_helpers::plan_json;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the plan json functions and classes.
#[inline]
pub fn register_plan_json(module: &Bound<'_, PyModule>) -> PyResult<()> {
    plan_json::register(module)
}
