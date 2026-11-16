//! Register the sql tests functions and classes.

use crate::analysis::_helpers::sql_tests::extract_sql_tests_json;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the sql tests functions and classes.
pub fn register_sql_tests(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(extract_sql_tests_json, module)?)?;
    Ok(())
}
