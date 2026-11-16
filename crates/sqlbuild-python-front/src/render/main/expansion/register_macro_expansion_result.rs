//! Register the macro expansion result functions and classes.

use crate::render::_helpers::expansion::macro_expansion_result;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the macro expansion result functions and classes.
#[inline]
pub fn register_macro_expansion_result(module: &Bound<'_, PyModule>) -> PyResult<()> {
    macro_expansion_result::register(module)
}
