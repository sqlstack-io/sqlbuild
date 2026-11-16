//! Detect a project-level adapter.py without importing it.

use crate::discovery::_helpers::records::declaration_records;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Detect a project-level adapter.py without importing it.
#[inline]
pub fn discover_adapter_file<'py>(project_dir: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    declaration_records::discover_adapter_file(project_dir)
}
