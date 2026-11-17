//! Register the scalars functions and classes.

use crate::cursor::_helpers::scalars;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the scalars functions and classes.
#[inline]
pub fn register_scalars(module: &Bound<'_, PyModule>) -> PyResult<()> {
    scalars::register(module)
}
