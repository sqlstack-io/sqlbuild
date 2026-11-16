//! Discover generic and singular audit SQL files in their project-wide and scoped roles.

use crate::discovery::_helpers::records::declaration_records;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::types::PyTuple;

/// Discover generic and singular audit SQL files in their project-wide and scoped roles.
#[inline]
pub fn discover_audit_files<'py>(
    project_dir: &Bound<'py, PyAny>,
    on_fault: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyTuple>> {
    declaration_records::discover_audit_files(project_dir, on_fault)
}
