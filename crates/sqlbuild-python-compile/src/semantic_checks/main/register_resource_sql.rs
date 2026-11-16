//! Register the resource sql functions and classes.

use crate::semantic_checks::_helpers::recovery::resource_sql::resource_sql_diagnostics_of;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the resource sql functions and classes.
pub fn register_resource_sql(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(resource_sql_diagnostics_of, module)?)
}
