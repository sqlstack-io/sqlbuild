//! Register the observations state functions and classes.

use crate::native_host::_helpers::observations_state;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the observations state functions and classes.
#[inline]
pub fn register_observations_state(module: &Bound<'_, PyModule>) -> PyResult<()> {
    observations_state::register(module)
}
