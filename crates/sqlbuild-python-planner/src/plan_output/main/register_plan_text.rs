//! Register the plan text functions and classes.

use crate::plan_output::_helpers::plan_text;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the plan text functions and classes.
#[inline]
pub fn register_plan_text(module: &Bound<'_, PyModule>) -> PyResult<()> {
    plan_text::register(module)
}
