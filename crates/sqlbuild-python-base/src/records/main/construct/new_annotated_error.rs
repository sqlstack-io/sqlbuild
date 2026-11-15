//! A raisable native `id` error carrying `message`, with each `(name, value)` attribute set on it.

use crate::records::_helpers::native_classes::construct;
use crate::records::types::ErrorId;
use pyo3::prelude::PyErr;
use pyo3::prelude::Python;

/// A raisable native `id` error carrying `message`, with each `(name, value)` attribute set on it.
#[inline]
pub fn new_annotated_error(
    py: Python<'_>,
    id: ErrorId,
    message: String,
    attributes: &[(&str, String)],
) -> PyErr {
    construct::new_annotated_error(py, id, message, attributes)
}
