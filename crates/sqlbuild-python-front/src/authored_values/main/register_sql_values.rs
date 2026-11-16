//! Register the sql values functions and classes.

use crate::authored_values::_helpers::sql_values;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the sql values functions and classes.
#[inline]
pub fn register_sql_values(module: &Bound<'_, PyModule>) -> PyResult<()> {
    sql_values::register(module)
}
