//! Register the evaluation functions and classes.

use crate::rules::_helpers::checking::evaluation::run_custom_host_json;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the evaluation functions and classes.
pub fn register_evaluation(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(run_custom_host_json, module)?)?;
    crate::rules::_helpers::checking::rows::register(module)
}
