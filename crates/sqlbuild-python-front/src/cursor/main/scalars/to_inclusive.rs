//! An exclusive bound moved back by one cursor unit.

use crate::cursor::_helpers::scalars::stepped;
use pyo3::prelude::{Bound, PyAny, PyResult};
use pyo3::pyfunction;
use sqlbuild_cursor::algebra::main::exclusive_to_inclusive::exclusive_to_inclusive;

/// An exclusive bound moved back by one cursor unit.
#[pyfunction(name = "cursor_exclusive_to_inclusive")]
#[pyo3(signature = (*, value, grain))]
pub fn to_inclusive<'py>(
    value: &Bound<'py, PyAny>,
    grain: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    stepped(value, grain, exclusive_to_inclusive)
}
