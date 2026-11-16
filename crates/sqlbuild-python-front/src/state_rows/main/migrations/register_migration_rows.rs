//! Register the migration rows functions and classes.

use crate::state_rows::_helpers::migrations::migration_rows::create_sql;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the migration rows functions and classes.
pub fn register_migration_rows(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(create_sql, module)?)?;
    Ok(())
}
