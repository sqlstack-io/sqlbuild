//! Compile project roles.

use crate::graph::models::ProjectSelection;
use crate::project_snapshot::_helpers::snapshot::snapshot_roles;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use sqlbuild_analysis::graph::models::ProjectGraph;
use sqlbuild_orchestration::snapshots::models::SnapshotInput;
use std::path::Path;

/// Compile project roles.
#[inline]
pub fn compile_project_roles(
    py: Python<'_>,
    input: SnapshotInput,
    (project_dir, graph, selected): (&Path, &ProjectGraph, &ProjectSelection),
) -> PyResult<SnapshotInput> {
    snapshot_roles::compile_project_roles(py, input, (project_dir, graph, selected))
}
