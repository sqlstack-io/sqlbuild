//! A raisable native `id` error carrying `message`, the diagnostic `code` and optional `help`.

use crate::records::_helpers::native_classes::construct;
use crate::records::types::ErrorId;
use pyo3::prelude::PyErr;
use pyo3::prelude::Python;

/// A raisable native `id` error carrying `message`, the diagnostic `code` and optional `help`.
#[inline]
pub fn new_helped_error(
    py: Python<'_>,
    id: ErrorId,
    (message, code): (String, &str),
    help: Option<&str>,
) -> PyErr {
    construct::new_helped_error(py, id, (message, code), help)
}
