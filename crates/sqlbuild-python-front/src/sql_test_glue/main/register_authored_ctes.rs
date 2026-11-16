//! Register the authored ctes functions and classes.

use crate::sql_test_glue::_helpers::authored_ctes;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the authored ctes functions and classes.
#[inline]
pub fn register_authored_ctes(module: &Bound<'_, PyModule>) -> PyResult<()> {
    authored_ctes::register(module)
}
