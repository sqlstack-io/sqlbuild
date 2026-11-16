//! Discover global and scoped project macro Python files.

use crate::discovery::_helpers::records::declaration_records::discovered;
use crate::discovery::_helpers::records::declaration_records::faults;
use pyo3::prelude::{Bound, PyAny, PyResult};
use pyo3::pyfunction;
use pyo3::types::PyTuple;
use sqlbuild_discovery::declaration_files::models::CollectionKind;

/// Discover global and scoped project macro Python files.
#[pyfunction]
#[pyo3(signature = (*, project_dir, isolate_declaration_kind=false))]
pub fn discover_macro_files<'py>(
    project_dir: &Bound<'py, PyAny>,
    isolate_declaration_kind: bool,
) -> PyResult<Bound<'py, PyTuple>> {
    discovered(
        &faults(project_dir, None),
        CollectionKind::Macros,
        isolate_declaration_kind,
    )
}
