//! A raisable native `id` error carrying `message` and the diagnostic `code`.

use crate::records::_helpers::native_classes::construct;
use crate::records::types::ErrorId;
use pyo3::prelude::PyErr;
use pyo3::prelude::Python;

/// A raisable native `id` error carrying `message` and the diagnostic `code`.
#[inline]
pub fn new_error(py: Python<'_>, id: ErrorId, message: String, code: &str) -> PyErr {
    construct::new_error(py, id, message, code)
}
