//! Register the trackers functions and classes.

use crate::diagnostics::_helpers::trackers;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the trackers functions and classes.
#[inline]
pub fn register_trackers(module: &Bound<'_, PyModule>) -> PyResult<()> {
    trackers::register(module)
}
