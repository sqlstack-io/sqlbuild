//! Register the diff functions and classes.

use crate::compile_driver::_helpers::stages::diff::resolve_diff_model_names;
use crate::compile_driver::main::stages::run_diff_pipeline::run_diff_pipeline;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the diff functions and classes.
pub fn register_diff(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(resolve_diff_model_names, module)?)?;
    module.add_function(wrap_pyfunction!(run_diff_pipeline, module)?)
}
