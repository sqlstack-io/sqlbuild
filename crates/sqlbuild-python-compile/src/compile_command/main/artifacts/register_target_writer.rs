//! Register the target writer functions and classes.

use crate::compile_command::_helpers::artifacts::target_writer::write_static_compile_target;
use crate::compile_command::main::artifacts::write_compile_target::write_compile_target;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the target writer functions and classes.
pub fn register_target_writer(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(write_static_compile_target, module)?)?;
    module.add_function(wrap_pyfunction!(write_compile_target, module)?)
}
