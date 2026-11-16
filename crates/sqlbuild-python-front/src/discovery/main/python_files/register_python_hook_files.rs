//! Register the python hook files functions and classes.

use crate::discovery::_helpers::python_files::python_hook_files;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the python hook files functions and classes.
#[inline]
pub fn register_python_hook_files(module: &Bound<'_, PyModule>) -> PyResult<()> {
    python_hook_files::register(module)
}
