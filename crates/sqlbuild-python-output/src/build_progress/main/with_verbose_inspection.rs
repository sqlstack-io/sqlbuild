//! Run `body`, writing each inspection read and then a total to stderr while `enabled`.

use crate::build_progress::_helpers::progress_text;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// Run `body`, writing each inspection read and then a total to stderr while `enabled`.
#[inline]
pub fn with_verbose_inspection<'py, T>(
    py: Python<'py>,
    enabled: bool,
    body: impl FnOnce() -> PyResult<T>,
) -> PyResult<T> {
    progress_text::with_verbose_inspection(py, enabled, body)
}
