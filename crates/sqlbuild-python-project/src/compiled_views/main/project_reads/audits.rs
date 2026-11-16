//! The project's compiled audits.

use crate::compiled_views::_helpers::project_reads;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// The project's compiled audits.
#[inline]
pub fn audits<'py>(project: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    project_reads::audits(project)
}
