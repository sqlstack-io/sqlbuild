//! Register the state io functions and classes.

use crate::freshness::_helpers::state_io;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the state io functions and classes.
#[inline]
pub fn register_state_io(module: &Bound<'_, PyModule>) -> PyResult<()> {
    state_io::register(module)
}
