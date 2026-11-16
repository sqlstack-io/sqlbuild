//! One cursor scalar, preserving date-ness and timestamp offsets.

use crate::cursor::main::scalars::parse_value::parse_value;
use pyo3::prelude::{Bound, PyAny, PyResult};
use pyo3::pyfunction;

/// One cursor scalar, preserving date-ness and timestamp offsets.
#[pyfunction(name = "cursor_parse")]
#[pyo3(signature = (*, raw, cursor_type))]
pub fn parse<'py>(
    raw: &Bound<'py, PyAny>,
    cursor_type: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    parse_value(raw, cursor_type)
}
