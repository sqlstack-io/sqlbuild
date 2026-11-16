//! Register the store keys functions and classes.

use crate::macro_bridge::_helpers::store_keys;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the store keys functions and classes.
#[inline]
pub fn register_store_keys(module: &Bound<'_, PyModule>) -> PyResult<()> {
    store_keys::register(module)
}
