//! Register the assembly functions and classes.

use crate::compile_driver::_helpers::stages::assembly::assemble_project;
use crate::compile_driver::_helpers::stages::assembly::assembly_stage_inputs;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the assembly functions and classes.
pub fn register_assembly(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(assemble_project, module)?)?;
    module.add_function(wrap_pyfunction!(assembly_stage_inputs, module)?)
}
