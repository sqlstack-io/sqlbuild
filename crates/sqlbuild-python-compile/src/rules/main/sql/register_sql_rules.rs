//! Register the sql rules functions and classes.

use crate::rules::_helpers::sql::sql_rules::run_planned_sql_rules;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the sql rules functions and classes.
pub fn register_sql_rules(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(run_planned_sql_rules, module)?)?;
    Ok(())
}
