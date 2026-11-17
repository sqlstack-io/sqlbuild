//! Register the startup inputs functions and classes.

use crate::compile_command::_helpers::invocation::startup_inputs::startup_input_stamps;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the startup inputs functions and classes.
pub fn register_startup_inputs(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(startup_input_stamps, module)?)?;
    Ok(())
}
