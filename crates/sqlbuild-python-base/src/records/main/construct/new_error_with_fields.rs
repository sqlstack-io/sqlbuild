//! The native `id` exception raised with `message`, its `code` and `help`, and declared keyword fields.

use crate::records::_helpers::native_classes::construct;
use crate::records::types::ErrorId;
use pyo3::prelude::{Bound, PyAny, PyErr, Python};

/// The native `id` exception with `message`, the given `code` and `help`, and its declared `fields`, or the error raising it.
#[inline]
pub fn new_error_with_fields<'py>(
    py: Python<'py>,
    id: ErrorId,
    (message, code, help): (&str, Option<&str>, Option<&str>),
    fields: &[(&str, Bound<'py, PyAny>)],
) -> PyErr {
    construct::exception_with_fields(
        construct::error_class(py, id),
        message,
        (code, help),
        fields,
    )
}
