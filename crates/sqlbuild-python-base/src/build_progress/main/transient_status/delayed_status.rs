//! Run `body`, drawing a stderr spinner named `message` only once it has run for `delay`.

use crate::build_progress::_helpers::transient_status;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use std::time::Duration;

/// Run `body`, drawing a stderr spinner named `message` only once it has run for `delay`.
#[inline]
pub fn delayed_status<'py, T>(
    py: Python<'py>,
    (message, delay): (&str, Duration),
    body: impl FnOnce() -> PyResult<T>,
) -> PyResult<T> {
    transient_status::delayed_status(py, (message, delay), body)
}
