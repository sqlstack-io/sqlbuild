//! Register the python module files functions and classes.

use crate::discovery::_helpers::python_files::python_module_files;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the python module files functions and classes.
#[inline]
pub fn register_python_module_files(module: &Bound<'_, PyModule>) -> PyResult<()> {
    python_module_files::register(module)
}
