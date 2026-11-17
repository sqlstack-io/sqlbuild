//! Register the python strings functions and classes.

use crate::refactor::_helpers::python_strings;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the python strings functions and classes.
#[inline]
pub fn register_python_strings(module: &Bound<'_, PyModule>) -> PyResult<()> {
    python_strings::register(module)
}
