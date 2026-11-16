//! The process-wide coordinator of the one live transient line.

use crate::build_progress::_helpers::terminal;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// The process-wide coordinator of the one live transient line.
#[inline]
pub fn shared_lines(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    terminal::shared_lines(py)
}
