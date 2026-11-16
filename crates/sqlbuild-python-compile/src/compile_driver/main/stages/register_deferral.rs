//! Register the deferral functions and classes.

use crate::compile_driver::main::stages::build_deferred_locations::build_deferred_locations;
use crate::compile_driver::main::stages::gather_deferred_relations::gather_deferred_relations;
use crate::compile_driver::main::stages::resolve_deferred_target_config::resolve_deferred_target_config;
use crate::compile_driver::main::stages::resolve_model_deferral_inputs::resolve_model_deferral_inputs;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the deferral functions and classes.
pub fn register_deferral(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(build_deferred_locations, module)?)?;
    module.add_function(wrap_pyfunction!(resolve_deferred_target_config, module)?)?;
    module.add_function(wrap_pyfunction!(gather_deferred_relations, module)?)?;
    module.add_function(wrap_pyfunction!(resolve_model_deferral_inputs, module)?)
}
