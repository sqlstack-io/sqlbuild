//! Write `value` to the stream and flush it.

use crate::build_command::_helpers::support;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Write `value` to the stream and flush it.
#[inline]
pub fn emit(stream: &Bound<'_, PyAny>, value: &str) -> PyResult<()> {
    support::emit(stream, value)
}
