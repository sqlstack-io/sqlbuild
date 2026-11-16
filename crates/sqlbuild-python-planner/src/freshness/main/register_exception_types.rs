//! Register the exception types functions and classes.

use crate::freshness::_helpers::exception_types;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the exception types functions and classes.
#[inline]
pub fn register_exception_types(module: &Bound<'_, PyModule>) -> PyResult<()> {
    exception_types::register(module)
}
