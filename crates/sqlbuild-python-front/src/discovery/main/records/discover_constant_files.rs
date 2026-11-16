//! Discover global and scoped constant declaration files.

use crate::discovery::_helpers::records::declaration_records;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::types::PyTuple;

/// Discover global and scoped constant declaration files.
#[inline]
pub fn discover_constant_files<'py>(
    project_dir: &Bound<'py, PyAny>,
    on_fault: Option<Bound<'py, PyAny>>,
    isolate_declaration_kind: bool,
) -> PyResult<Bound<'py, PyTuple>> {
    declaration_records::discover_constant_files(project_dir, on_fault, isolate_declaration_kind)
}
