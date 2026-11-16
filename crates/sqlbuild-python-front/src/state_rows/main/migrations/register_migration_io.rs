//! Register the migration io functions and classes.

use crate::state_rows::_helpers::migrations::migration_io;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the migration io functions and classes.
#[inline]
pub fn register_migration_io(module: &Bound<'_, PyModule>) -> PyResult<()> {
    migration_io::register(module)
}
