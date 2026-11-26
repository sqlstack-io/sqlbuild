//! Register the functions that run loads.

use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult, wrap_pyfunction};

use crate::bindings::_helpers::run_binding::run_sqlload_project;

/// Register the functions that run loads.
pub fn register_loads(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(run_sqlload_project, module)?)
}
