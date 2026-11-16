//! Discover named SQL lifecycle hooks in their project-wide and scoped roles.

use crate::discovery::_helpers::records::declaration_records;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::types::PyTuple;

/// Discover named SQL lifecycle hooks in their project-wide and scoped roles.
#[inline]
pub fn discover_sql_hook_files<'py>(
    project_dir: &Bound<'py, PyAny>,
    on_fault: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyTuple>> {
    declaration_records::discover_sql_hook_files(project_dir, on_fault)
}
