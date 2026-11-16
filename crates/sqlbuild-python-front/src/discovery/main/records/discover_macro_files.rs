//! Discover global and scoped project macro Python files.

use crate::discovery::_helpers::records::declaration_records;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::types::PyTuple;

/// Discover global and scoped project macro Python files.
#[inline]
pub fn discover_macro_files<'py>(
    project_dir: &Bound<'py, PyAny>,
    isolate_declaration_kind: bool,
) -> PyResult<Bound<'py, PyTuple>> {
    declaration_records::discover_macro_files(project_dir, isolate_declaration_kind)
}
