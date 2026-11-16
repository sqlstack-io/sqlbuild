//! `moment` (a `date` or `datetime`) moved by `span` in `direction`, keeping its type and `tzinfo`.

use crate::cursor::_helpers::durations;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use sqlbuild_cursor::algebra::models::DurationSpan;

/// `moment` (a `date` or `datetime`) moved by `span` in `direction`, keeping its type and `tzinfo`.
#[inline]
pub fn shifted_moment<'py>(
    moment: &Bound<'py, PyAny>,
    span: &DurationSpan,
    direction: i64,
) -> PyResult<Bound<'py, PyAny>> {
    durations::shifted_moment(moment, span, direction)
}
