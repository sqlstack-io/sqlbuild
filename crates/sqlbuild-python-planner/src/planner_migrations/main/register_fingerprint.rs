//! Register the fingerprint functions and classes.

use crate::planner_migrations::_helpers::projections::fingerprint;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the fingerprint functions and classes.
#[inline]
pub fn register_fingerprint(module: &Bound<'_, PyModule>) -> PyResult<()> {
    fingerprint::register(module)
}
