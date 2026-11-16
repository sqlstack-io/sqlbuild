//! The project's compiled models, in compile order.

use crate::compiled_views::_helpers::project::project_reads;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// The project's compiled models, in compile order.
#[inline]
pub fn compiled_models<'py>(project: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    project_reads::models(project)
}
