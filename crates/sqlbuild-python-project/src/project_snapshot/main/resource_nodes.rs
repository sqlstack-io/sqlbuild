//! Resource nodes.

use crate::compiled_project::models::NativeCompiledProject;
use crate::compiled_views::models::NativeProjectView;
use crate::project_snapshot::_helpers::snapshot::snapshot_resources;
use pyo3::prelude::{PyResult, Python};
use sqlbuild_orchestration::snapshots::models::SnapshotNode;
use std::collections::BTreeMap;

/// Resource nodes.
#[inline]
pub fn resource_nodes(
    py: Python<'_>,
    project: &NativeProjectView,
    owner: &NativeCompiledProject,
    nodes: BTreeMap<String, SnapshotNode>,
) -> PyResult<BTreeMap<String, SnapshotNode>> {
    snapshot_resources::resource_nodes(py, project, owner, nodes)
}
