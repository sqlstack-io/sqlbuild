//! How a run that raised an error ended.

use pyo3::PyErr;
use pyo3::exceptions::PyKeyboardInterrupt;
use pyo3::prelude::Python;
use sqlbuild_state::runs::models::RunEnding;

/// How a run that raised `error` ended: an interrupt, including a worker's cancel, or an error.
pub fn raised_ending(py: Python<'_>, error: &PyErr) -> RunEnding {
    if error.is_instance_of::<PyKeyboardInterrupt>(py) {
        RunEnding::Interrupted
    } else {
        RunEnding::Raised
    }
}
