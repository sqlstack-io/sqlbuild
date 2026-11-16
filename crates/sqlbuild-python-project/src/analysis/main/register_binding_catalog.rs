//! Register the binding catalog functions and classes.

use crate::analysis::_helpers::binding_catalog::schema_validations;
use crate::analysis::models::BindingCatalog;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the binding catalog functions and classes.
pub fn register_binding_catalog(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<BindingCatalog>()?;
    module.add_function(wrap_pyfunction!(schema_validations, module)?)?;
    Ok(())
}
