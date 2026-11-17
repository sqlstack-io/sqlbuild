//! Raise `CursorAlgebraError` unless the ends form a valid aligned interval.

use crate::cursor::models::CursorSession;
use pyo3::prelude::{Bound, PyAny, PyResult};
use pyo3::pyfunction;
use sqlbuild_cursor::algebra::main::validate_interval::validate_interval;
use sqlbuild_cursor::algebra::models::Interval;

/// Raise `CursorAlgebraError` unless the ends form a valid aligned interval.
#[pyfunction(name = "validate_cursor_interval")]
#[pyo3(signature = (*, start, end, grain))]
pub fn interval_validation(
    start: &Bound<'_, PyAny>,
    end: &Bound<'_, PyAny>,
    grain: &Bound<'_, PyAny>,
) -> PyResult<()> {
    let session: CursorSession<'_> = CursorSession::new(start.py())?;
    let interval = Interval {
        start: session.scalar(start)?,
        end: session.scalar(end)?,
        grain: session.grain(grain)?,
    };
    session.checked(validate_interval(&interval, &session))
}
