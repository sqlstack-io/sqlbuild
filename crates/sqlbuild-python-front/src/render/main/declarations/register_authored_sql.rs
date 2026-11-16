//! Register the authored sql functions and classes.

use crate::render::_helpers::declarations::authored_sql;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the authored sql functions and classes.
#[inline]
pub fn register_authored_sql(module: &Bound<'_, PyModule>) -> PyResult<()> {
    authored_sql::register(module)
}
