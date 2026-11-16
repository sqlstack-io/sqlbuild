//! Register the discovery functions and classes.

use crate::compile_driver::_helpers::stages::discovery::build_discovered_project_inputs;
use crate::compile_driver::main::stages::discover_project_inputs::discover_project_inputs;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the discovery functions and classes.
pub fn register_discovery(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(discover_project_inputs, module)?)?;
    module.add_function(wrap_pyfunction!(build_discovered_project_inputs, module)?)
}
