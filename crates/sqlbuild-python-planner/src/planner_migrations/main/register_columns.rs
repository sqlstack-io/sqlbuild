//! Register the columns functions and classes.

use crate::planner_migrations::_helpers::column::columns;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the columns functions and classes.
#[inline]
pub fn register_columns(module: &Bound<'_, PyModule>) -> PyResult<()> {
    columns::register(module)
}
