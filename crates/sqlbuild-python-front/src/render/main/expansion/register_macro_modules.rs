//! Register the macro modules functions and classes.

use crate::render::_helpers::expansion::macro_modules;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the macro modules functions and classes.
#[inline]
pub fn register_macro_modules(module: &Bound<'_, PyModule>) -> PyResult<()> {
    macro_modules::register(module)
}
