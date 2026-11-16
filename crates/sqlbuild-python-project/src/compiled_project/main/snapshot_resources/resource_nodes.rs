//! Resource nodes.

use crate::compiled_project::_helpers::snapshot_resources;
use crate::compiled_project::models::NativeCompiledProject;
use crate::compiled_views::models::NativeProjectView;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
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
