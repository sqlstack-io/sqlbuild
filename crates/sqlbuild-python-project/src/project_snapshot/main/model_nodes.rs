//! Each model's snapshot node, and the `columns` document of ranges kept out of node identity.

use crate::compiled_project::models::NativeCompiledProject;
use crate::compiled_views::models::NativeProjectView;
use crate::project_snapshot::_helpers::snapshot::snapshot_models;
use pyo3::prelude::{PyResult, Python};
use serde_json::{Map, Value};
use sqlbuild_orchestration::snapshots::models::SnapshotNode;
use std::collections::BTreeMap;

/// Each model's snapshot node, and the `columns` document of ranges kept out of node identity.
#[inline]
pub fn model_nodes(
    py: Python<'_>,
    project: &NativeProjectView,
    owner: &NativeCompiledProject,
    nodes: BTreeMap<String, SnapshotNode>,
) -> PyResult<(BTreeMap<String, SnapshotNode>, Map<String, Value>)> {
    snapshot_models::model_nodes(py, project, owner, nodes)
}
