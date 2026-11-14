//! Orchestration declarations read natively, raised as the Python exceptions discovery raises.

use std::path::PathBuf;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use sqlbuild_discovery::orchestration_files::main::orchestration_inventory::orchestration_inventory;
use sqlbuild_discovery::orchestration_files::models::{
    OrchestrationFailure, OrchestrationInventory,
};
use sqlbuild_discovery::tree::models::ProjectTree;

use crate::bindings::_helpers::discovery::orchestration_values::check_orchestration_values;
use crate::bindings::_helpers::discovery::raised::{
    failure_error, relative_read_error, stage_error,
};

/// Every job, job group, calendar and sensor of `project_dir`, or `OrchestrationDeclarationError` (D017).
pub(crate) fn orchestration_declarations(
    project_dir: &Bound<'_, PyAny>,
) -> PyResult<OrchestrationInventory> {
    let py: Python<'_> = project_dir.py();
    let root: PathBuf = project_dir.extract()?;
    let tree = ProjectTree::new(&root);
    let version = py.version_info();
    let inventory = py.detach(|| orchestration_inventory(&tree, (version.major, version.minor)));
    let inventory = inventory.map_err(|failure| orchestration_error(&tree, project_dir, failure))?;
    check_orchestration_values(py, &inventory).map_err(|failure| failure_error(py, failure))?;
    Ok(inventory)
}

fn orchestration_error(
    tree: &ProjectTree,
    project_dir: &Bound<'_, PyAny>,
    failure: OrchestrationFailure,
) -> PyErr {
    match failure {
        OrchestrationFailure::Declaration(failure) => failure_error(project_dir.py(), failure),
        OrchestrationFailure::Unreadable {
            relative_path,
            failure,
        } => relative_read_error((tree, project_dir), relative_path, failure),
        OrchestrationFailure::Stage(failure) => stage_error(tree, project_dir, failure),
    }
}
