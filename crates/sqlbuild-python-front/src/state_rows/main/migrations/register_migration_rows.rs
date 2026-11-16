//! Register the migration rows functions and classes.

use crate::state_rows::_helpers::migrations::migration_rows;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the migration rows functions and classes.
#[inline]
pub fn register_migration_rows(module: &Bound<'_, PyModule>) -> PyResult<()> {
    migration_rows::register(module)
}
