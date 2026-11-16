//! Register the latest rows functions and classes.

use crate::fingerprints::_helpers::latest_rows;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the latest rows functions and classes.
#[inline]
pub fn register_latest_rows(module: &Bound<'_, PyModule>) -> PyResult<()> {
    latest_rows::register(module)
}
