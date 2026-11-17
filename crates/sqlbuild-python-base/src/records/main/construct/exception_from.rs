//! The exception `class` with `message` and the given `code` and `help`.

use crate::records::_helpers::native_classes::construct;
use pyo3::prelude::{Bound, PyAny, PyErr, PyResult};

/// The exception `class` with `message` and the given `code` and `help`, or the error raising it.
#[inline]
pub fn exception_from(
    class: PyResult<Bound<'_, PyAny>>,
    message: &str,
    keywords: (Option<&str>, Option<&str>),
) -> PyErr {
    construct::exception_from(class, message, keywords)
}
