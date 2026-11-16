//! Register the queries functions and classes.

use crate::analysis::_helpers::queries::parse_warehouse_type;
use crate::analysis::_helpers::queries::query_fingerprint;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;
use sqlbuild_python_project::analysis::main::register_binding_catalog::register_binding_catalog;
use sqlbuild_python_project::references::main::register_explicit_references::register_explicit_references;

/// Register the queries functions and classes.
pub fn register_queries(module: &Bound<'_, PyModule>) -> PyResult<()> {
    register_binding_catalog(module)?;
    crate::analysis::_helpers::required_columns::register(module)?;
    register_explicit_references(module)?;
    crate::analysis::_helpers::binding_locations::register(module)?;
    module.add_function(wrap_pyfunction!(query_fingerprint, module)?)?;
    module.add_function(wrap_pyfunction!(parse_warehouse_type, module)?)?;
    Ok(())
}
