//! Register the macro arguments functions and classes.

use crate::render::_helpers::calls::macro_arguments::parse_macro_arguments;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the macro arguments functions and classes.
pub fn register_macro_arguments(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(parse_macro_arguments, module)?)?;
    Ok(())
}
