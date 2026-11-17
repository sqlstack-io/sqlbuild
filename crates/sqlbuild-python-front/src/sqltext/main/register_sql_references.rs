//! Register the sql references functions and classes.

use crate::sqltext::_helpers::sql_references::SqlReferenceScanner;
use crate::sqltext::_helpers::sql_references::scan_sql_reference_calls;
use crate::sqltext::main::extract_references::extract_references;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the sql references functions and classes.
pub fn register_sql_references(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<SqlReferenceScanner>()?;
    module.add_function(wrap_pyfunction!(scan_sql_reference_calls, module)?)?;
    module.add_function(wrap_pyfunction!(extract_references, module)?)?;
    Ok(())
}
