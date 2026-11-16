//! The project's compiled functions.

use crate::compiled_views::_helpers::project_reads;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// The project's compiled functions.
#[inline]
pub fn functions<'py>(project: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    project_reads::functions(project)
}
