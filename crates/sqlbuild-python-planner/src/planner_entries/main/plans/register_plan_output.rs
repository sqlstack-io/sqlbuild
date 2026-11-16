//! Register the plan output functions and classes.

use crate::planner_entries::_helpers::plans::plan_output::build_selected_test_entries;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;
use sqlbuild_python_project::sql_test_glue::main::sql_test_model_chain_names_by_key_function::sql_test_model_chain_names_by_key_function;

/// Register the plan output functions and classes.
pub fn register_plan_output(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(sql_test_model_chain_names_by_key_function(module)?)?;
    module.add_function(wrap_pyfunction!(build_selected_test_entries, module)?)
}
