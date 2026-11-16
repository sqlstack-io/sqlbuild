//! The projector installed for this invocation, when there is one.

use crate::build_progress::_helpers::progress;
use crate::build_progress::models::NativeProgressProjector;
use pyo3::prelude::Py;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// The projector installed for this invocation, when there is one.
#[inline]
pub fn current_projector(py: Python<'_>) -> PyResult<Option<Py<NativeProgressProjector>>> {
    progress::current_projector(py)
}
