//! Return the projector installed for the current CLI invocation.

use crate::build_progress::_helpers::progress;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// Return the projector installed for the current CLI invocation.
#[inline]
pub fn current_native_progress_projector(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    progress::current_native_progress_projector(py)
}
