//! The project's compiled SQL tests.

use crate::compiled_views::_helpers::project::project_reads;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// The project's compiled SQL tests.
#[inline]
pub fn sql_tests<'py>(project: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    project_reads::sql_tests(project)
}
