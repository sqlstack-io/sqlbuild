//! The native `id` exception raised with `message` and the `code` and `help` keywords that are given.

use crate::records::_helpers::native_classes::construct;
use crate::records::types::ErrorId;
use pyo3::prelude::PyErr;
use pyo3::prelude::Python;

/// The native `id` exception with `message` and the given `code` and `help`, or the error raising it.
#[inline]
pub fn new_error(
    py: Python<'_>,
    id: ErrorId,
    message: &str,
    keywords: (Option<&str>, Option<&str>),
) -> PyErr {
    construct::new_error(py, id, message, keywords)
}
