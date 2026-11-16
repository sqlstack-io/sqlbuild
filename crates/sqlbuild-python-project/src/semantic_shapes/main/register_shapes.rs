//! Register the shapes functions and classes.

use crate::semantic_shapes::_helpers::shapes;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the shapes functions and classes.
#[inline]
pub fn register_shapes(module: &Bound<'_, PyModule>) -> PyResult<()> {
    shapes::register(module)
}
