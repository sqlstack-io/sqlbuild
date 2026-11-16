//! Register the state tables functions and classes.

use crate::state_rows::_helpers::tables::state_tables;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the state tables functions and classes.
#[inline]
pub fn register_state_tables(module: &Bound<'_, PyModule>) -> PyResult<()> {
    state_tables::register(module)
}
