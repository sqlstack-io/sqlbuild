//! Register the resource identities functions and classes.

use crate::contracts::_helpers::resource_identities;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the resource identities functions and classes.
#[inline]
pub fn register_resource_identities(module: &Bound<'_, PyModule>) -> PyResult<()> {
    resource_identities::register(module)
}
