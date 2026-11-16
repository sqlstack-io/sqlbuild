//! The Python error a capture serializer failure stands for.

use crate::frontier::_helpers::encoder;
use pyo3::prelude::PyErr;
use sqlbuild_core::json::errors::JsonEmitError;

/// The Python error a capture serializer failure stands for.
#[inline]
pub fn emit_error(error: JsonEmitError) -> PyErr {
    encoder::emit_error(error)
}
