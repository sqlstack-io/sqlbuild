//! Register the runtime target functions and classes.

use crate::compile_outputs::_helpers::runtime_target::write_runtime_target_binding;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the runtime target functions and classes.
pub fn register_runtime_target(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(write_runtime_target_binding, module)?)
}
