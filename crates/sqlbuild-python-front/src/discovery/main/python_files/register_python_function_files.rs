//! Register the python function files functions and classes.

use crate::discovery::main::python_files::discover_python_function_files::discover_python_function_files;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the python function files functions and classes.
pub fn register_python_function_files(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(discover_python_function_files, module)?)
}
