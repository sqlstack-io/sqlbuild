//! `-1`, `0` or `1` as `left` orders against `right`.

use crate::cursor::models::CursorSession;
use pyo3::prelude::{Bound, PyAny, PyResult};
use pyo3::pyfunction;
use sqlbuild_cursor::algebra::main::compare::compare;
use sqlbuild_cursor::algebra::models::Scalar;
use std::cmp::Ordering;

/// `-1`, `0` or `1` as `left` orders against `right`.
#[pyfunction(name = "cursor_compare")]
#[pyo3(signature = (*, left, right))]
pub fn compare_values(left: &Bound<'_, PyAny>, right: &Bound<'_, PyAny>) -> PyResult<i8> {
    let session: CursorSession<'_> = CursorSession::new(left.py())?;
    let (left, right): (Scalar, Scalar) = (session.scalar(left)?, session.scalar(right)?);
    Ok(match session.checked(compare(&left, &right, &session))? {
        Ordering::Less => -1,
        Ordering::Equal => 0,
        Ordering::Greater => 1,
    })
}
