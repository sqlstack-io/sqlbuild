//! The project's compiled seeds.

use crate::compiled_views::_helpers::project::project_reads;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// The project's compiled seeds.
#[inline]
pub fn seeds<'py>(project: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    project_reads::seeds(project)
}
