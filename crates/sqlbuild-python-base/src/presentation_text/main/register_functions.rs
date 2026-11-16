//! Register the functions functions and classes.

use crate::presentation_text::_helpers::functions;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the functions functions and classes.
#[inline]
pub fn register_functions(module: &Bound<'_, PyModule>) -> PyResult<()> {
    functions::register(module)
}
