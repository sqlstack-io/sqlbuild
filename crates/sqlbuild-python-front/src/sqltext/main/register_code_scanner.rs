//! Register the code scanner functions and classes.

use crate::sqltext::_helpers::code_scanner;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the code scanner functions and classes.
#[inline]
pub fn register_code_scanner(module: &Bound<'_, PyModule>) -> PyResult<()> {
    code_scanner::register(module)
}
