//! `(years, months, days, hours, minutes, seconds)` of a duration like `1y6mo` or `30d`.

use crate::cursor::_helpers::durations;
use pyo3::prelude::Bound;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::types::PyTuple;

/// `(years, months, days, hours, minutes, seconds)` of a duration like `1y6mo` or `30d`.
#[inline]
pub fn parse_cursor_duration<'py>(
    py: Python<'py>,
    value: &str,
) -> PyResult<Option<Bound<'py, PyTuple>>> {
    durations::parse_cursor_duration(py, value)
}
