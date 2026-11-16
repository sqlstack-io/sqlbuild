//! The calendars and sensors of the inventory's modules, imported by publication and by sensor evaluation.

use crate::compiled_project::_helpers::snapshot_declarations;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use sqlbuild_discovery::orchestration_files::models::OrchestrationInventory;
use std::path::Path;

/// The calendars and sensors of the inventory's modules, imported by publication and by sensor evaluation.
#[inline]
pub fn python_declarations<'py>(
    py: Python<'py>,
    project_dir: &Path,
    inventory: &OrchestrationInventory,
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    snapshot_declarations::python_declarations(py, project_dir, inventory)
}
