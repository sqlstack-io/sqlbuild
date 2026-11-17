//! The current build's phase timing tracker, when one is recording.

use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use sqlbuild_python_base::diagnostics::main::current_tracker::current_tracker;

/// The current build's phase timing tracker, when one is recording.
pub fn timing_tracker(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    current_tracker(py)
}
