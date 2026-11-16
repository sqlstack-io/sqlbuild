//! Register the authored ctes functions and classes.

use crate::sql_test_glue::main::validate_authored_sql_test_ctes::validate_authored_sql_test_ctes;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the authored ctes functions and classes.
pub fn register_authored_ctes(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(validate_authored_sql_test_ctes, module)?)
}
