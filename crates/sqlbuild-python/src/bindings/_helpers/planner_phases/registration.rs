//! Register the native execution-planning phases.

use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use sqlbuild_python_project::planner_scope::main::resolve_planner_scopes_function::resolve_planner_scopes_function;
use sqlbuild_python_planner::planner_phases::main::outputs::detect_stale_warning_changes_of_function::detect_stale_warning_changes_of_function;
use sqlbuild_python_planner::planner_phases::main::outputs::planner_identity_work_info_function::planner_identity_work_info_function;
use sqlbuild_python_planner::planner_phases::main::rebuilds::check_full_rebuild_allowed_function::check_full_rebuild_allowed_function;
use sqlbuild_python_planner::planner_phases::main::rebuilds::describe_full_rebuild_cause_function::describe_full_rebuild_cause_function;
use sqlbuild_python_planner::planner_phases::main::rebuilds::plan_retention_function::plan_retention_function;
use sqlbuild_python_planner::planner_phases::main::rebuilds::plan_table_types_function::plan_table_types_function;
use sqlbuild_python_planner::planner_phases::main::rebuilds::table_type_copy_name_function::table_type_copy_name_function;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<sqlbuild_python_planner::planner_phases::models::PlannerRuntime>()?;
    module.add_class::<sqlbuild_python_planner::planner_phases::models::PlannerWarehouseState>()?;
    module.add_class::<sqlbuild_python_project::planner_scope::models::PlannerScopeResolution>()?;
    module
        .add_class::<sqlbuild_python_planner::planner_phases::models::PlannerIdentityContext>()?;
    module.add_function(resolve_planner_scopes_function(module)?)?;
    module.add_function(planner_identity_work_info_function(module)?)?;
    module.add_function(detect_stale_warning_changes_of_function(module)?)?;
    module.add_function(describe_full_rebuild_cause_function(module)?)?;
    module.add_function(check_full_rebuild_allowed_function(module)?)?;
    module.add_function(table_type_copy_name_function(module)?)?;
    module.add_function(plan_table_types_function(module)?)?;
    module.add_function(plan_retention_function(module)?)?;
    Ok(())
}
