//! A native status reporter writing to `stream`.

use crate::build_progress::_helpers::transient_status;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// A native status reporter writing to `stream`.
#[inline]
pub fn status_reporter<'py>(
    stream: &Bound<'py, PyAny>,
    use_color: bool,
) -> PyResult<Bound<'py, PyAny>> {
    transient_status::status_reporter(stream, use_color)
}
