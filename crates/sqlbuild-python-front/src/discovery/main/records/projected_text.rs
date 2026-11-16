//! `text` as the Python string `cache` shares with every reader, built on first read.

use crate::discovery::_helpers::records::model_file_record;
use pyo3::prelude::Bound;
use pyo3::prelude::Py;
use pyo3::prelude::Python;
use pyo3::sync::PyOnceLock;
use pyo3::types::PyString;

/// `text` as the Python string `cache` shares with every reader, built on first read.
#[inline]
pub fn projected_text<'py>(
    py: Python<'py>,
    cache: &PyOnceLock<Py<PyString>>,
    text: &str,
) -> Bound<'py, PyString> {
    model_file_record::projected_text(py, cache, text)
}
