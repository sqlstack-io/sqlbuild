//! A sentinel's token, or a scalar rendered.

use crate::cursor::_helpers::scalars;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// A sentinel's token, or a scalar rendered.
#[inline]
pub fn sentinel_to_token(sentinel: &Bound<'_, PyAny>) -> PyResult<String> {
    scalars::sentinel_to_token(sentinel)
}
