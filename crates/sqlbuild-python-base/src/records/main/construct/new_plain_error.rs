//! The native `id` exception raised with `message`, or the error looking it up or raising it.

use crate::records::_helpers::native_classes::construct;
use crate::records::types::ErrorId;
use pyo3::PyErr;
use pyo3::prelude::Python;

/// The native `id` exception raised with `message`, or the error looking it up or raising it.
#[inline]
pub fn new_plain_error(py: Python<'_>, id: ErrorId, message: &str) -> PyErr {
    construct::new_plain_error(py, id, message)
}
