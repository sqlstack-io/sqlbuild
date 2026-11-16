//! The current native progress projector, if a projection is active.

use crate::build_progress::_helpers::terminal;
use pyo3::prelude::Py;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// The current native progress projector, if a projection is active.
#[inline]
pub fn projector(py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
    terminal::projector(py)
}
