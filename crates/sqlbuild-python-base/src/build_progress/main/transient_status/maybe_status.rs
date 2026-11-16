//! Run `body` under a stderr spinner when `enabled` and stderr is an interactive terminal.

use crate::build_progress::_helpers::transient_status;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// Run `body` under a stderr spinner when `enabled` and stderr is an interactive terminal.
#[inline]
pub fn maybe_status<'py, T>(
    py: Python<'py>,
    (message, enabled): (&str, bool),
    body: impl FnOnce() -> PyResult<T>,
) -> PyResult<T> {
    transient_status::maybe_status(py, (message, enabled), body)
}
