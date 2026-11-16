//! Register the command functions and classes.

use crate::compile_driver::_helpers::stages::command::defer_compile_teardown_to_exit;
use crate::compile_driver::main::stages::run_compile_command::run_compile_command;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the command functions and classes.
pub fn register_command(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(defer_compile_teardown_to_exit, module)?)?;
    module.add_function(wrap_pyfunction!(run_compile_command, module)?)
}
