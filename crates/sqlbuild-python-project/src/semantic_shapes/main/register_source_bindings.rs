//! Register the source bindings functions and classes.

use crate::semantic_shapes::_helpers::source_bindings::source_binding_diagnostics;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the source bindings functions and classes.
pub fn register_source_bindings(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(source_binding_diagnostics, module)?)
}
