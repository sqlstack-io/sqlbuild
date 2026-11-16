//! Register the intervals functions and classes.

use crate::cursor::_helpers::intervals;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the intervals functions and classes.
#[inline]
pub fn register_intervals(module: &Bound<'_, PyModule>) -> PyResult<()> {
    intervals::register(module)
}
