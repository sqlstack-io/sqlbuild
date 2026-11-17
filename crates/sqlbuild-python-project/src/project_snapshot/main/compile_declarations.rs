//! Compile declarations.

use crate::graph::models::ProjectSelection;
use crate::project_snapshot::_helpers::snapshot::snapshot_declarations;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use sqlbuild_analysis::graph::models::ProjectGraph;
use sqlbuild_discovery::orchestration_files::models::OrchestrationInventory;
<<<<<<< HEAD
use sqlbuild_orchestration::declarations::models::ResolvedConcurrency;
=======
>>>>>>> origin/main
use sqlbuild_orchestration::snapshots::models::SnapshotInput;
use std::path::Path;

/// Compile declarations.
#[inline]
pub fn compile_declarations(
    py: Python<'_>,
    input: SnapshotInput,
<<<<<<< HEAD
    ((project_dir, graph, selected), concurrency): (
        (&Path, &ProjectGraph, &ProjectSelection),
        ResolvedConcurrency,
    ),
=======
    (project_dir, graph, selected): (&Path, &ProjectGraph, &ProjectSelection),
>>>>>>> origin/main
    (inventory, declarations): (&OrchestrationInventory, Vec<Bound<'_, PyAny>>),
) -> PyResult<SnapshotInput> {
    snapshot_declarations::compile_declarations(
        py,
        input,
<<<<<<< HEAD
        ((project_dir, graph, selected), concurrency),
=======
        (project_dir, graph, selected),
>>>>>>> origin/main
        (inventory, declarations),
    )
}
