//! `raw` unchanged when it is already a typed value, else parsed.

use crate::cursor::main::scalars::parse_value::parse_value;
use crate::cursor::models::CursorSession;
use pyo3::prelude::{Bound, PyAny, PyResult};
use sqlbuild_cursor::algebra::models::Scalar;

/// `raw` unchanged when it is already a typed value, else parsed.
pub fn as_scalar<'py>(
    session: &CursorSession<'py>,
    raw: &Bound<'py, PyAny>,
    cursor_type: &Bound<'py, PyAny>,
) -> PyResult<Scalar> {
    if session.scalar_typed(raw)? {
        return session.scalar(raw);
    }
    session.scalar(&parse_value(raw, cursor_type)?)
}
