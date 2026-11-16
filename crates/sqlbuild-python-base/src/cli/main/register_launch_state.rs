//! Register the launch state functions and classes.

use crate::cli::_helpers::launch_state;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the launch state functions and classes.
#[inline]
pub fn register_launch_state(module: &Bound<'_, PyModule>) -> PyResult<()> {
    launch_state::register(module)
}
