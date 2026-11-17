//! `text` as the Python string `cache` shares with every reader, built on first read.

use pyo3::prelude::{Bound, Py, Python};
use pyo3::sync::PyOnceLock;
use pyo3::types::PyString;

/// `text` as the Python string `cache` shares with every reader, built on first read.
pub fn projected_text<'py>(
    py: Python<'py>,
    cache: &PyOnceLock<Py<PyString>>,
    text: &str,
) -> Bound<'py, PyString> {
    cache
        .get_or_init(py, || PyString::new(py, text).unbind())
        .bind(py)
        .clone()
}
