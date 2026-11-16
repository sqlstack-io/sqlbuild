//! Register the render functions and classes.

use crate::auditing::main::render_audit_sql::render_audit_sql;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the render functions and classes.
pub fn register_render(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(render_audit_sql, module)?)?;
    Ok(())
}
