//! Discovery values that may be dates or timestamps, or `None` when they do not parse.

use crate::cursor::main::scalars::parse_value::parse_value;
use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use pyo3::pyfunction;

/// Discovery values that may be dates or timestamps, or `None` when they do not parse.
#[pyfunction(name = "cursor_try_parse")]
#[pyo3(signature = (*, raw, cursor_type))]
pub fn try_parse<'py>(
    raw: &Bound<'py, PyAny>,
    cursor_type: &Bound<'py, PyAny>,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let py: Python<'py> = raw.py();
    match parse_value(raw, cursor_type) {
        Ok(value) => Ok(Some(value)),
        Err(error)
            if error.is_instance_of::<PyTypeError>(py)
                || error.is_instance_of::<PyValueError>(py) =>
        {
            Ok(None)
        }
        Err(error) => Err(error),
    }
}
