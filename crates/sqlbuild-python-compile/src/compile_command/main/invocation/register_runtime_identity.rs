//! Register the runtime identity functions and classes.

use crate::compile_command::_helpers::invocation::runtime_identity::interpreter_identity;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the runtime identity functions and classes.
pub fn register_runtime_identity(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(interpreter_identity, module)?)
}
