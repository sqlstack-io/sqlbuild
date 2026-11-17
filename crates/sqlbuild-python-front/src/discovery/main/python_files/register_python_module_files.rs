//! Register the python module files functions and classes.

use crate::discovery::_helpers::python_files::python_module_files::discover_python_module_files;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the python module files functions and classes.
pub fn register_python_module_files(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(discover_python_module_files, module)?)
}
