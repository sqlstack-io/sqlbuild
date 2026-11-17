//! Refuse a compile whose job groups have members that wait on each other, as `OrchestrationDeclarationError` (D017).

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_analysis::graph::models::ProjectGraph;
use sqlbuild_python_base::records::main::fields::field::field;
use sqlbuild_python_front::discovery::main::sources::orchestration_declarations::orchestration_declarations;
use sqlbuild_python_front::discovery::main::sources::orchestration_failure::orchestration_failure;

use crate::graph::models::ProjectSelection;
use crate::project_snapshot::_helpers::snapshot::snapshot_group_cycles::{
    group_cycle_failures, has_groups,
};

/// Every cycle in every job group declared for the project's target, in one error; reads Python nodes only when a group has two or more jobs.
pub fn job_group_cycles(
    project_dir: &Bound<'_, PyAny>,
    (project, discovered_inputs): (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
    graph: &ProjectGraph,
) -> PyResult<()> {
    let py = project_dir.py();
    let inventory = orchestration_declarations(project_dir)?;
    let target: String = field(project, "effective_target_name")?
        .extract::<Option<String>>()?
        .unwrap_or_default();
    if !has_groups(&inventory, &target) {
        return Ok(());
    }
    let selected = ProjectSelection::new(project, discovered_inputs)?;
    let failures: Vec<String> = group_cycle_failures(&inventory, &target, (graph, &selected));
    if failures.is_empty() {
        return Ok(());
    }
    Err(orchestration_failure(py, failures.join("\n")))
}
