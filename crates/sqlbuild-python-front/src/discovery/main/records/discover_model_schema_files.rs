//! Discover project-wide and scoped reusable model schemas.

use crate::discovery::_helpers::records::declaration_records;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::types::PyTuple;

/// Discover project-wide and scoped reusable model schemas.
#[inline]
pub fn discover_model_schema_files<'py>(
    project_dir: &Bound<'py, PyAny>,
    on_fault: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyTuple>> {
    declaration_records::discover_model_schema_files(project_dir, on_fault)
}
