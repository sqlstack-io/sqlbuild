//! Register the refs functions and classes.

use crate::planner_resolution::_helpers::references::refs::resolve_dbt_ref_references;
use crate::planner_resolution::_helpers::references::refs::resolve_ref_references;
use crate::planner_resolution::_helpers::references::refs::resolve_table_function_references;
use crate::planner_resolution::_helpers::references::refs::resolve_udf_references;
use crate::planner_resolution::main::references::apply_deferred_locations::apply_deferred_locations;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the refs functions and classes.
pub fn register_refs(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(resolve_ref_references, module)?)?;
    module.add_function(wrap_pyfunction!(resolve_dbt_ref_references, module)?)?;
    module.add_function(wrap_pyfunction!(resolve_udf_references, module)?)?;
    module.add_function(wrap_pyfunction!(resolve_table_function_references, module)?)?;
    module.add_function(wrap_pyfunction!(apply_deferred_locations, module)?)?;
    Ok(())
}
