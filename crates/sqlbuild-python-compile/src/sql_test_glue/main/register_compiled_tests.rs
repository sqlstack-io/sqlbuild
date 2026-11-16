//! Register the compiled tests functions and classes.

use crate::sql_test_glue::main::assemble_native_sql_tests::assemble_native_sql_tests;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the compiled tests functions and classes.
pub fn register_compiled_tests(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(assemble_native_sql_tests, module)?)?;
    Ok(())
}
