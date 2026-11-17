//! The text of `text`; lone surrogates, which UTF-8 cannot hold, are kept as `\\xNN` or `\\uXXXX` escapes.

use pyo3::prelude::{Bound, PyResult};
use pyo3::types::{PyString, PyStringMethods};
use sqlbuild_python_front::discovery::main::snapshots::escaped::escaped;

/// The text of `text`; lone surrogates, which UTF-8 cannot hold, are kept as `\\xNN` or `\\uXXXX` escapes.
pub fn escaped_text(text: &Bound<'_, PyString>) -> PyResult<String> {
    match text.to_str() {
        Ok(valid) => Ok(valid.to_owned()),
        Err(_) => escaped(text),
    }
}
