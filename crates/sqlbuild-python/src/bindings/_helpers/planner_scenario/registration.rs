//! Register scenario planning bindings.

use crate::bindings::_helpers::planner_scenario::artifacts;
use crate::bindings::_helpers::planner_scenario::cli;
use crate::bindings::_helpers::planner_scenario::execution;
use crate::bindings::_helpers::planner_scenario::fixtures;
use crate::bindings::_helpers::planner_scenario::graph;
use crate::bindings::_helpers::planner_scenario::relations;
use crate::bindings::_helpers::planner_scenario::sql;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(
        execution::build_scenario_execution_plan,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        fixtures::build_scenario_fixture_plans,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(sql::resolve_scenario_check_sql, module)?)?;
    module.add_function(wrap_pyfunction!(
        artifacts::compute_scenario_hash_prefix,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        artifacts::build_scenario_hash_index,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        artifacts::build_scenario_artifact_name,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        artifacts::parse_scenario_artifact_physical_name,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        artifacts::is_scenario_artifact_physical_name,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        artifacts::build_scenario_relation_map,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(graph::plan_scenario_graph, module)?)?;
    module.add_function(wrap_pyfunction!(
        relations::build_scenario_relation_plan_of,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(cli::build_cli_scenario_plan, module)?)?;
    Ok(())
}
