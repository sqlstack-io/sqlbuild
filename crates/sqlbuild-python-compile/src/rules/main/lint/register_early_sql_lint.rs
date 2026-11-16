//! Register the early sql lint functions and classes.

use crate::rules::models::EarlySqlLint;
use crate::rules::models::PreparedSqlLint;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};

/// Register the early sql lint functions and classes.
pub fn register_early_sql_lint(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<EarlySqlLint>()?;
    module.add_class::<PreparedSqlLint>()?;
    Ok(())
}
