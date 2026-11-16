//! An operator-declared inclusive bound advanced by one cursor unit.

use crate::cursor::_helpers::scalars::stepped;
use pyo3::prelude::{Bound, PyAny, PyResult};
use pyo3::pyfunction;
use sqlbuild_cursor::algebra::main::inclusive_to_exclusive::inclusive_to_exclusive;

/// An operator-declared inclusive bound advanced by one cursor unit.
#[pyfunction(name = "cursor_inclusive_to_exclusive")]
#[pyo3(signature = (*, value, grain))]
pub fn to_exclusive<'py>(
    value: &Bound<'py, PyAny>,
    grain: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    stepped(value, grain, inclusive_to_exclusive)
}
