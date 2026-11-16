//! Register the native execution-planning phases.

use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<crate::bindings::_helpers::planner_phases::models::PlannerRuntime>()?;
    module
        .add_class::<crate::bindings::_helpers::planner_phases::models::PlannerWarehouseState>()?;
    module
        .add_class::<crate::bindings::_helpers::planner_phases::models::PlannerScopeResolution>()?;
    module
        .add_class::<crate::bindings::_helpers::planner_phases::models::PlannerIdentityContext>()?;
    module.add_function(wrap_pyfunction!(
        crate::bindings::_helpers::planner_phases::scopes::resolve_planner_scopes,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        crate::bindings::_helpers::planner_phases::identities::planner_identity_work_info,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        crate::bindings::_helpers::planner_phases::identities::detect_stale_warning_changes_of,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        crate::bindings::_helpers::planner_phases::full_refresh::describe_full_rebuild_cause,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        crate::bindings::_helpers::planner_phases::full_refresh::check_full_rebuild_allowed,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        crate::bindings::_helpers::planner_phases::retention::table_type_copy_name,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        crate::bindings::_helpers::planner_phases::retention::plan_table_types,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        crate::bindings::_helpers::planner_phases::retention::plan_retention,
        module
    )?)?;
    Ok(())
}
