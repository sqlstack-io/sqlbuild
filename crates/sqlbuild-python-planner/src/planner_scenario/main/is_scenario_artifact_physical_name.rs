//! Is scenario artifact physical name.

use pyo3::prelude::pyfunction;
use sqlbuild_analysis::planner_scenario::main::parse_artifact_name;

#[pyfunction]
pub fn is_scenario_artifact_physical_name(name: &str) -> bool {
    parse_artifact_name::parse_artifact_name(name).is_some()
}
