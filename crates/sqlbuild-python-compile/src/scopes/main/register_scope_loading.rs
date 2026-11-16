//! Register the scope loading functions and classes.

use crate::scopes::main::load_or_build_scope_index::load_or_build_scope_index;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the scope loading functions and classes.
pub fn register_scope_loading(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(load_or_build_scope_index, module)?)
}
