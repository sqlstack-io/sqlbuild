//! Register the render functions and classes.

use crate::auditing::_helpers::render;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the render functions and classes.
#[inline]
pub fn register_render(module: &Bound<'_, PyModule>) -> PyResult<()> {
    render::register(module)
}
