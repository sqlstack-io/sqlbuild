//! Register the projection functions and classes.

use crate::microbatches::_helpers::projection;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the projection functions and classes.
#[inline]
pub fn register_projection(module: &Bound<'_, PyModule>) -> PyResult<()> {
    projection::register(module)
}
