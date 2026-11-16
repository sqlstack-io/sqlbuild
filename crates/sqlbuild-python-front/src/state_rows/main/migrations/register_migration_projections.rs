//! Register the migration projections functions and classes.

use crate::state_rows::_helpers::migrations::migration_projections;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the migration projections functions and classes.
#[inline]
pub fn register_migration_projections(module: &Bound<'_, PyModule>) -> PyResult<()> {
    migration_projections::register(module)
}
