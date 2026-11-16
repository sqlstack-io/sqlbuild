//! The active build timing tracker, or None.

use crate::diagnostics::_helpers::trackers;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// The active build timing tracker, or None.
#[inline]
pub fn current_tracker(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    trackers::current_tracker(py)
}
