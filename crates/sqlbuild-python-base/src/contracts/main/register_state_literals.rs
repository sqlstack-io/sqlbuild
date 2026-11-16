//! Register the state literals functions and classes.

use crate::contracts::_helpers::state_literals;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the state literals functions and classes.
#[inline]
pub fn register_state_literals(module: &Bound<'_, PyModule>) -> PyResult<()> {
    state_literals::register(module)
}
