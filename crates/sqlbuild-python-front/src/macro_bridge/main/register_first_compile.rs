//! Register the first compile functions and classes.

use crate::macro_bridge::_helpers::first_compile::claim_first_compile;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the first compile functions and classes.
pub fn register_first_compile(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(claim_first_compile, module)?)
}
