//! Register the macro arguments functions and classes.

use crate::render::_helpers::calls::macro_arguments;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the macro arguments functions and classes.
#[inline]
pub fn register_macro_arguments(module: &Bound<'_, PyModule>) -> PyResult<()> {
    macro_arguments::register(module)
}
