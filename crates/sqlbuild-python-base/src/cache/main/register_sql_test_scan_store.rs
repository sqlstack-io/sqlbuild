//! Register the sql test scan store functions and classes.

use crate::cache::_helpers::sql_test_scan_store;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the sql test scan store functions and classes.
#[inline]
pub fn register_sql_test_scan_store(module: &Bound<'_, PyModule>) -> PyResult<()> {
    sql_test_scan_store::register(module)
}
