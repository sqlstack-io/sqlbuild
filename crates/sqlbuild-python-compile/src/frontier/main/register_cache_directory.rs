//! Register the cache directory functions and classes.

use crate::frontier::_helpers::cache_directory::compiler_cache_directory;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the cache directory functions and classes.
pub fn register_cache_directory(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(compiler_cache_directory, module)?)
}
