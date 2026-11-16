//! Register the python hook files functions and classes.

use crate::discovery::_helpers::python_files::python_hook_files::discover_python_hook_files;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the python hook files functions and classes.
pub fn register_python_hook_files(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(discover_python_hook_files, module)?)
}
