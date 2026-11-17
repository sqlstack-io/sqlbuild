//! Register the inputs functions and classes.

use crate::compile_driver::_helpers::setup::inputs::build_compile_inputs;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the inputs functions and classes.
pub fn register_inputs(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(build_compile_inputs, module)?)
}
