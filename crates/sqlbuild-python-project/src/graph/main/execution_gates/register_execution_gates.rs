//! Register the execution gates functions and classes.

use crate::graph::_helpers::execution_gates;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the execution gates functions and classes.
#[inline]
pub fn register_execution_gates(module: &Bound<'_, PyModule>) -> PyResult<()> {
    execution_gates::register(module)
}
