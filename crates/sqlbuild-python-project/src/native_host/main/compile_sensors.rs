//! Compile sensors.

use crate::graph::models::ProjectSelection;
use crate::native_host::_helpers::orchestration_triggers;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use sqlbuild_analysis::graph::models::ProjectGraph;
use sqlbuild_orchestration::declarations::models::Definitions;
use sqlbuild_orchestration::snapshots::models::SnapshotInput;
use sqlbuild_python_base::orchestration_reads::models::AuthoredSensor;
use std::path::Path;

/// Compile sensors.
#[inline]
pub fn compile_sensors(
    py: Python<'_>,
    (project_dir, input, graph): (&Path, &SnapshotInput, (&ProjectGraph, &ProjectSelection)),
    authored: Vec<AuthoredSensor>,
    definitions: Definitions,
) -> PyResult<Definitions> {
    orchestration_triggers::compile_sensors(py, (project_dir, input, graph), authored, definitions)
}
