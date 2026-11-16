//! `-1`, `0` or `1` as `left` orders against `right`.

use crate::cursor::_helpers::scalars;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// `-1`, `0` or `1` as `left` orders against `right`.
#[inline]
pub fn compare_values(left: &Bound<'_, PyAny>, right: &Bound<'_, PyAny>) -> PyResult<i8> {
    scalars::compare_values(left, right)
}
