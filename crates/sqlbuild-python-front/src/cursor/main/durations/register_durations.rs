//! Register the durations functions and classes.

use crate::cursor::_helpers::durations;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the durations functions and classes.
#[inline]
pub fn register_durations(module: &Bound<'_, PyModule>) -> PyResult<()> {
    durations::register(module)
}
