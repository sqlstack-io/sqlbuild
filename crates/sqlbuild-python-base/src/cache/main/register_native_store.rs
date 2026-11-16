//! Register the native store functions and classes.

use crate::cache::_helpers::native_store;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the native store functions and classes.
#[inline]
pub fn register_native_store(module: &Bound<'_, PyModule>) -> PyResult<()> {
    native_store::register(module)
}
