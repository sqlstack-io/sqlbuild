//! Register the command plans functions and classes.

use crate::planner_entries::_helpers::plans::command_plans::build_seed_command_plan;
use crate::planner_entries::_helpers::plans::command_plans::build_test_command_plan;
use crate::planner_entries::_helpers::plans::command_plans::read_selected_seed_fingerprints;
use crate::planner_entries::main::plans::build_audit_command_plan::build_audit_command_plan;
use crate::planner_entries::main::plans::resolve_static_command_scope::resolve_static_command_scope;
use crate::planner_entries::main::plans::resolve_static_relation_context::resolve_static_relation_context;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the command plans functions and classes.
pub fn register_command_plans(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(resolve_static_command_scope, module)?)?;
    module.add_function(wrap_pyfunction!(resolve_static_relation_context, module)?)?;
    module.add_function(wrap_pyfunction!(build_audit_command_plan, module)?)?;
    module.add_function(wrap_pyfunction!(build_test_command_plan, module)?)?;
    module.add_function(wrap_pyfunction!(build_seed_command_plan, module)?)?;
    module.add_function(wrap_pyfunction!(read_selected_seed_fingerprints, module)?)
}
