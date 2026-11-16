//! The project with `changes`; a native view shares its owner's models.

use crate::compiled_views::_helpers::project::records::with_changes;
use pyo3::prelude::{Bound, PyAny, PyResult};
use pyo3::pyfunction;
use pyo3::types::PyDict;
use sqlbuild_python_base::records::types::RecordId;

/// The project with `changes`; a native view shares its owner's models.
#[pyfunction]
#[pyo3(signature = (*, project, **changes))]
pub fn project_with<'py>(
    project: &Bound<'py, PyAny>,
    changes: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    with_changes(project, RecordId::CompiledProject, changes)
}
