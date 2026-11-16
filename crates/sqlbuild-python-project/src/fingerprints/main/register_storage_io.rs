//! Register the storage io functions and classes.

use crate::fingerprints::_helpers::storage_io;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the storage io functions and classes.
#[inline]
pub fn register_storage_io(module: &Bound<'_, PyModule>) -> PyResult<()> {
    storage_io::register(module)
}
