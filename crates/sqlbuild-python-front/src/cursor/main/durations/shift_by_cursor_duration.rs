//! `moment` moved forward (`direction` 1) or back (`direction` -1) by `duration`.

use crate::cursor::_helpers::durations::duration_span;
use crate::cursor::main::durations::shifted_moment::shifted_moment;
use pyo3::prelude::{Bound, PyAny, PyResult};
use pyo3::pyfunction;

/// `moment` moved forward (`direction` 1) or back (`direction` -1) by `duration`.
#[pyfunction]
#[pyo3(signature = (*, moment, duration, direction))]
pub fn shift_by_cursor_duration<'py>(
    moment: &Bound<'py, PyAny>,
    duration: &Bound<'py, PyAny>,
    direction: i64,
) -> PyResult<Bound<'py, PyAny>> {
    shifted_moment(moment, &duration_span(duration)?, direction)
}
