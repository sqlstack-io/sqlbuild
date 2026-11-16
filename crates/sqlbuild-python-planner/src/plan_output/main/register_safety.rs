//! Register the safety functions and classes.

use crate::plan_output::_helpers::safety;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the safety functions and classes.
#[inline]
pub fn register_safety(module: &Bound<'_, PyModule>) -> PyResult<()> {
    safety::register(module)
}
