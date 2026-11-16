//! Python strings for each item of an iterable of `str`.

use crate::planner_selection::_helpers::carrier;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::types::PyString;

/// Python strings for each item of an iterable of `str`.
#[inline]
pub fn py_strings<'py>(values: &Bound<'py, PyAny>) -> PyResult<Vec<Bound<'py, PyString>>> {
    carrier::py_strings(values)
}
