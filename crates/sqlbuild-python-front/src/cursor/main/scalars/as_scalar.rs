//! `raw` unchanged when it is already a typed value, else parsed.

use crate::cursor::_helpers::scalars;
use crate::cursor::models::CursorSession;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use sqlbuild_cursor::algebra::models::Scalar;

/// `raw` unchanged when it is already a typed value, else parsed.
#[inline]
pub fn as_scalar<'py>(
    session: &CursorSession<'py>,
    raw: &Bound<'py, PyAny>,
    cursor_type: &Bound<'py, PyAny>,
) -> PyResult<Scalar> {
    scalars::as_scalar(session, raw, cursor_type)
}
