//! The canonical partition containing an observed physical value.

use crate::cursor::models::CursorSession;
use pyo3::prelude::{Bound, PyAny, PyResult};
use pyo3::pyfunction;
use sqlbuild_cursor::algebra::main::observed_partition::observed_partition;
use sqlbuild_cursor::algebra::models::{Interval, Scalar};

/// The canonical partition containing an observed physical value.
#[pyfunction(name = "cursor_observed_partition")]
#[pyo3(signature = (*, value, grain))]
pub fn partition<'py>(
    value: &Bound<'py, PyAny>,
    grain: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let session: CursorSession<'py> = CursorSession::new(value.py())?;
    let scalar: Scalar = session.scalar(value)?;
    let interval: Interval = session.checked(observed_partition(&scalar, session.grain(grain)?))?;
    session.python_interval(&interval)
}
