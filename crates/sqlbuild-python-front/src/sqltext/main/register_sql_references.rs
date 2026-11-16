//! Register the sql references functions and classes.

use crate::sqltext::_helpers::sql_references;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the sql references functions and classes.
#[inline]
pub fn register_sql_references(module: &Bound<'_, PyModule>) -> PyResult<()> {
    sql_references::register(module)
}
