//! The project's resource collection named `field`; any other field is read by attribute.

use crate::compiled_views::_helpers::project_reads;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// The project's resource collection named `field`; any other field is read by attribute.
#[inline]
pub fn collection<'py>(project: &Bound<'py, PyAny>, field: &str) -> PyResult<Bound<'py, PyAny>> {
    project_reads::collection(project, field)
}
