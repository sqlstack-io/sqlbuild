//! Register the column planning functions and classes.

use crate::planner_migrations::_helpers::column::column_planning;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the column planning functions and classes.
#[inline]
pub fn register_column_planning(module: &Bound<'_, PyModule>) -> PyResult<()> {
    column_planning::register(module)
}
