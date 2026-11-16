//! Register the shared native store functions and classes.

use pyo3::prelude::{Bound, PyModule, PyResult};

use sqlbuild_python_base::cache::main::register_native_store::register_native_store;
use sqlbuild_python_base::cache::main::register_sql_test_scan_store::register_sql_test_scan_store;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    register_native_store(module)?;
    register_sql_test_scan_store(module)?;
    Ok(())
}
