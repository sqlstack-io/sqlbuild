//! Register the macro expansion functions and classes.

use crate::render::_helpers::expansion::macro_expansion;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the macro expansion functions and classes.
#[inline]
pub fn register_macro_expansion(module: &Bound<'_, PyModule>) -> PyResult<()> {
    macro_expansion::register(module)
}
