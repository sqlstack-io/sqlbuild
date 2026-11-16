//! Register the orchestration functions and classes.

use crate::planner_resolution::_helpers::references::orchestration::resolve_model_hook_entries;
use crate::planner_resolution::_helpers::references::orchestration::resolve_model_sql_of;
use crate::planner_resolution::main::references::resolve_function_sql::resolve_function_sql;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the orchestration functions and classes.
pub fn register_orchestration(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(resolve_model_sql_of, module)?)?;
    module.add_function(wrap_pyfunction!(resolve_function_sql, module)?)?;
    module.add_function(wrap_pyfunction!(resolve_model_hook_entries, module)?)
}
