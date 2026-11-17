//! Register the analysis cache functions and classes.

use crate::compile_driver::_helpers::stages::analysis_cache::build_analysis_cache_context;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the analysis cache functions and classes.
pub fn register_analysis_cache(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(build_analysis_cache_context, module)?)
}
