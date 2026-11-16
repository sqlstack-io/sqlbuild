//! Register the sql test records functions and classes.

use crate::discovery::_helpers::records::sql_test_records;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the sql test records functions and classes.
#[inline]
pub fn register_sql_test_records(module: &Bound<'_, PyModule>) -> PyResult<()> {
    sql_test_records::register(module)
}
