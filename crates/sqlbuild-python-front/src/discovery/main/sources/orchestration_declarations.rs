//! Every job, job group, calendar and sensor of `project_dir`, or `OrchestrationDeclarationError` (D017).

use crate::discovery::_helpers::sources::orchestration_files;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use sqlbuild_discovery::orchestration_files::models::OrchestrationInventory;

/// Every job, job group, calendar and sensor of `project_dir`, or `OrchestrationDeclarationError` (D017).
#[inline]
pub fn orchestration_declarations(
    project_dir: &Bound<'_, PyAny>,
) -> PyResult<OrchestrationInventory> {
    orchestration_files::orchestration_declarations(project_dir)
}
