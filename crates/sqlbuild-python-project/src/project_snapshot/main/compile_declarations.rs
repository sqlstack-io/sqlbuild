//! Compile declarations.

use crate::graph::models::ProjectSelection;
use crate::project_snapshot::_helpers::snapshot::snapshot_declarations;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use sqlbuild_analysis::graph::models::ProjectGraph;
use sqlbuild_discovery::orchestration_files::models::OrchestrationInventory;
use sqlbuild_orchestration::snapshots::models::SnapshotInput;
use std::path::Path;

/// Compile declarations.
#[inline]
pub fn compile_declarations(
    py: Python<'_>,
    input: SnapshotInput,
    (project_dir, graph, selected): (&Path, &ProjectGraph, &ProjectSelection),
    (inventory, declarations): (&OrchestrationInventory, Vec<Bound<'_, PyAny>>),
) -> PyResult<SnapshotInput> {
    snapshot_declarations::compile_declarations(
        py,
        input,
        (project_dir, graph, selected),
        (inventory, declarations),
    )
}
