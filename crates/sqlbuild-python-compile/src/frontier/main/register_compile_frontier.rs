//! Register the compile frontier functions and classes.

use crate::frontier::_helpers::compile_frontier::compile_frontier;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the compile frontier functions and classes.
pub fn register_compile_frontier(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(compile_frontier, module)?)
}
