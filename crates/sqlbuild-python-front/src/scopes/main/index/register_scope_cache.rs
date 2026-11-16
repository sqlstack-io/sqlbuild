//! Register the scope cache functions and classes.

use crate::scopes::_helpers::index::scope_cache::scope_index_fingerprint;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the scope cache functions and classes.
pub fn register_scope_cache(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(scope_index_fingerprint, module)?)?;
    Ok(())
}
