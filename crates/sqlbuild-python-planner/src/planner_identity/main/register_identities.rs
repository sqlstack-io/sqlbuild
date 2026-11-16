//! Register the identities functions and classes.

use crate::planner_identity::_helpers::identities;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the identities functions and classes.
#[inline]
pub fn register_identities(module: &Bound<'_, PyModule>) -> PyResult<()> {
    identities::register(module)
}
