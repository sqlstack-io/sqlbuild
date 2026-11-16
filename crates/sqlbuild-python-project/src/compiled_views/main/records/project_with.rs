//! The project with `changes`; a native view shares its owner's models.

use crate::compiled_views::_helpers::records;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::types::PyDict;

/// The project with `changes`; a native view shares its owner's models.
#[inline]
pub fn project_with<'py>(
    project: &Bound<'py, PyAny>,
    changes: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    records::project_with(project, changes)
}
