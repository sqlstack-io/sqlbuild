//! The project's compiled sources.

use crate::compiled_views::_helpers::project::project_reads;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// The project's compiled sources.
#[inline]
pub fn sources<'py>(project: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    project_reads::sources(project)
}
