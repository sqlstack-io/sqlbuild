//! Orchestration declarations read natively, raised as the Python exceptions discovery raises.

use pyo3::prelude::{Bound, PyAny, PyErr};
use sqlbuild_discovery::orchestration_files::models::OrchestrationFailure;
use sqlbuild_discovery::tree::models::ProjectTree;

use crate::discovery::_helpers::failures::raised::{
    failure_error, relative_read_error, stage_error,
};

pub(crate) fn orchestration_error(
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
