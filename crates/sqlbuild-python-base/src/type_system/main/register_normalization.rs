//! Register the normalization functions and classes.

use crate::type_system::_helpers::normalization;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the normalization functions and classes.
#[inline]
pub fn register_normalization(module: &Bound<'_, PyModule>) -> PyResult<()> {
    normalization::register(module)
}
