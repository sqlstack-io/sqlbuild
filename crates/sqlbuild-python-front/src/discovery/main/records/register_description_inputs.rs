//! Register the description inputs functions and classes.

use crate::discovery::_helpers::records::description_inputs::discover_model_description_inputs;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the description inputs functions and classes.
pub fn register_description_inputs(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(discover_model_description_inputs, module)?)?;
    Ok(())
}
