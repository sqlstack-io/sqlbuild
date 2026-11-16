//! Register the resources functions and classes.

use crate::project_assembly::_helpers::resources::sql_syntax_error;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the resources functions and classes.
pub fn register_resources(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(sql_syntax_error, module)?)?;
    Ok(())
}
