//! Register the catalog functions and classes.

use crate::lifecycle_events::_helpers::catalog;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the catalog functions and classes.
#[inline]
pub fn register_catalog(module: &Bound<'_, PyModule>) -> PyResult<()> {
    catalog::register(module)
}
