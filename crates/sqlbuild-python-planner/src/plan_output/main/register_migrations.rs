//! Register the migrations functions and classes.

use crate::plan_output::_helpers::migrations;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the migrations functions and classes.
#[inline]
pub fn register_migrations(module: &Bound<'_, PyModule>) -> PyResult<()> {
    migrations::register(module)
}
