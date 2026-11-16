//! Register the hashing functions and classes.

use crate::planner_identity::_helpers::hashing;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the hashing functions and classes.
#[inline]
pub fn register_hashing(module: &Bound<'_, PyModule>) -> PyResult<()> {
    hashing::register(module)
}
