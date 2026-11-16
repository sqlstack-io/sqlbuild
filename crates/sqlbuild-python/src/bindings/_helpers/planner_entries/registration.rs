//! Register plan entry and plan output assembly entry points.

use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use sqlbuild_python_planner::planner_entries::main::entries::register_model_entries::register_model_entries;
use sqlbuild_python_planner::planner_entries::main::entries::register_relations::register_relations;
use sqlbuild_python_planner::planner_entries::main::plans::register_clone_plans::register_clone_plans;
use sqlbuild_python_planner::planner_entries::main::plans::register_command_plans::register_command_plans;
use sqlbuild_python_planner::planner_entries::main::plans::register_display_plan::register_display_plan;
use sqlbuild_python_planner::planner_entries::main::plans::register_execution_plan::register_execution_plan;
use sqlbuild_python_planner::planner_entries::main::plans::register_plan_output::register_plan_output;
use sqlbuild_python_planner::planner_entries::main::plans::register_test_planning::register_test_planning;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module
        .add_class::<sqlbuild_python_planner::planner_entries::models::PlannerRelationsContext>()?;
    module.add_class::<sqlbuild_python_planner::planner_entries::models::ModelPlanContext>()?;
    module.add_class::<sqlbuild_python_planner::planner_entries::models::PlanEntryBuildInputs>()?;
    register_clone_plans(module)?;
    register_command_plans(module)?;
    register_display_plan(module)?;
    register_execution_plan(module)?;
    register_model_entries(module)?;
    register_plan_output(module)?;
    register_relations(module)?;
    register_test_planning(module)
}
