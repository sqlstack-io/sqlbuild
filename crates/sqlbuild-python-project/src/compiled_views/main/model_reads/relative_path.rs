//! The model file's relative path.

use crate::compiled_views::_helpers::model_reads;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// The model file's relative path.
#[inline]
pub fn relative_path<'py>(model: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    model_reads::relative_path(model)
}
