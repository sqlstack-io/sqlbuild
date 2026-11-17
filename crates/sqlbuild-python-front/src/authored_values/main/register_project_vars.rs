//! Register the project vars functions and classes.

use crate::authored_values::main::render_project_var_text::render_project_var_text;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the project vars functions and classes.
pub fn register_project_vars(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(render_project_var_text, module)?)?;
    Ok(())
}
