//! Register the microbatches functions and classes.

use crate::planner_strategy::_helpers::microbatches;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the microbatches functions and classes.
#[inline]
pub fn register_microbatches(module: &Bound<'_, PyModule>) -> PyResult<()> {
    microbatches::register(module)
}
