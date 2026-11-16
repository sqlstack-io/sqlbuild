//! `value.timestamp()` of a `datetime`.

use crate::microbatches::_helpers::conversion;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// `value.timestamp()` of a `datetime`.
#[inline]
pub fn timestamp(value: &Bound<'_, PyAny>) -> PyResult<f64> {
    conversion::timestamp(value)
}
