//! Register the compile usages functions and classes.

use crate::scopes::_helpers::relationships::compile_usages::model_schema_scope_usages;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the compile usages functions and classes.
pub fn register_compile_usages(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(model_schema_scope_usages, module)?)?;
    Ok(())
}
