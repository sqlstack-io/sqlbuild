//! `moment` moved forward (`direction` 1) or back (`direction` -1) by `duration`.

use crate::cursor::_helpers::durations;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// `moment` moved forward (`direction` 1) or back (`direction` -1) by `duration`.
#[inline]
pub fn shift_by_cursor_duration<'py>(
    moment: &Bound<'py, PyAny>,
    duration: &Bound<'py, PyAny>,
    direction: i64,
) -> PyResult<Bound<'py, PyAny>> {
    durations::shift_by_cursor_duration(moment, duration, direction)
}
