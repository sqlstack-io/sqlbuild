//! Register the macro calls functions and classes.

use crate::render::_helpers::calls::macro_calls;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the macro calls functions and classes.
#[inline]
pub fn register_macro_calls(module: &Bound<'_, PyModule>) -> PyResult<()> {
    macro_calls::register(module)
}
