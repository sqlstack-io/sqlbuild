//! The terminal claim keywords of `function`: resource name, id, attempt id and attempt number.

use crate::build_progress::_helpers::progress;
use pyo3::prelude::{Bound, PyResult, Python};
use pyo3::types::PyDict;
use sqlbuild_core::terminal_events::models::TerminalRequest;

/// The terminal claim keywords of `function`: resource name, id, attempt id and attempt number.
#[inline]
pub fn terminal_request(
    py: Python<'_>,
    function: &str,
    options: Option<Bound<'_, PyDict>>,
) -> PyResult<TerminalRequest> {
    progress::terminal_request(py, function, options)
}
