//! Register the placeholders functions and classes.

use crate::sqltext::_helpers::placeholders::substitute_placeholder_defaults;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the placeholders functions and classes.
pub fn register_placeholders(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(substitute_placeholder_defaults, module)?)?;
    Ok(())
}
