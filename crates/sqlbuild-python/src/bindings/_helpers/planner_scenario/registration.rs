//! Register scenario planning bindings.

use sqlbuild_python_planner::planner_scenario::main::build_scenario_artifact_name_function::build_scenario_artifact_name_function;
use sqlbuild_python_planner::planner_scenario::main::build_scenario_hash_index_function::build_scenario_hash_index_function;
use sqlbuild_python_planner::planner_scenario::main::build_scenario_relation_map_function::build_scenario_relation_map_function;
use sqlbuild_python_planner::planner_scenario::main::compute_scenario_hash_prefix_function::compute_scenario_hash_prefix_function;
use sqlbuild_python_planner::planner_scenario::main::scenario_artifact_physical_name_check_function::scenario_artifact_physical_name_check_function;
use sqlbuild_python_planner::planner_scenario::main::parse_scenario_artifact_physical_name_function::parse_scenario_artifact_physical_name_function;
use sqlbuild_python_planner::planner_scenario::main::build_cli_scenario_plan_function::build_cli_scenario_plan_function;
use sqlbuild_python_planner::planner_scenario::main::build_scenario_execution_plan_function::build_scenario_execution_plan_function;
use sqlbuild_python_planner::planner_scenario::main::build_scenario_fixture_plans_function::build_scenario_fixture_plans_function;
use sqlbuild_python_planner::planner_scenario::main::plan_scenario_graph_function::plan_scenario_graph_function;
use sqlbuild_python_planner::planner_scenario::main::build_scenario_relation_plan_of_function::build_scenario_relation_plan_of_function;
use sqlbuild_python_planner::planner_scenario::main::resolve_scenario_check_sql_function::resolve_scenario_check_sql_function;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(build_scenario_execution_plan_function(module)?)?;
    module.add_function(build_scenario_fixture_plans_function(module)?)?;
    module.add_function(resolve_scenario_check_sql_function(module)?)?;
    module.add_function(compute_scenario_hash_prefix_function(module)?)?;
    module.add_function(build_scenario_hash_index_function(module)?)?;
    module.add_function(build_scenario_artifact_name_function(module)?)?;
    module.add_function(parse_scenario_artifact_physical_name_function(module)?)?;
    module.add_function(scenario_artifact_physical_name_check_function(module)?)?;
    module.add_function(build_scenario_relation_map_function(module)?)?;
    module.add_function(plan_scenario_graph_function(module)?)?;
    module.add_function(build_scenario_relation_plan_of_function(module)?)?;
    module.add_function(build_cli_scenario_plan_function(module)?)?;
    Ok(())
}
