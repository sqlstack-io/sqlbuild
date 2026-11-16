//! A Python `str`.

use pyo3::prelude::{Bound, PyAny, Python};
use pyo3::types::PyString;

/// A Python `str`.
pub fn text<'py>(py: Python<'py>, value: &str) -> Bound<'py, PyAny> {
    PyString::new(py, value).into_any()
}
