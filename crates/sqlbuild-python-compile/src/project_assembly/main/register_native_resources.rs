//! Register the native resources functions and classes.

use crate::project_assembly::main::assemble_native_project_resources::assemble_native_project_resources;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the native resources functions and classes.
pub fn register_native_resources(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(assemble_native_project_resources, module)?)?;
    Ok(())
}
