//! Discover generic and singular audit SQL files in their project-wide and scoped roles.

use crate::discovery::_helpers::records::declaration_records::discovered;
use crate::discovery::_helpers::records::declaration_records::faults;
use pyo3::prelude::{Bound, PyAny, PyResult};
use pyo3::pyfunction;
use pyo3::types::PyTuple;
use sqlbuild_discovery::declaration_files::models::CollectionKind;

/// Discover generic and singular audit SQL files in their project-wide and scoped roles.
#[pyfunction]
#[pyo3(signature = (*, project_dir, on_fault=None))]
pub fn discover_audit_files<'py>(
    project_dir: &Bound<'py, PyAny>,
    on_fault: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyTuple>> {
    discovered(
        &faults(project_dir, on_fault),
        CollectionKind::Audits,
        false,
    )
}
