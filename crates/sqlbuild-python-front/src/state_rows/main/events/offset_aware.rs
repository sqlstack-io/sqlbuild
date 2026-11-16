//! Whether `value` has a `tzinfo` that reports an offset.

use crate::state_rows::_helpers::events::timestamps;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Whether `value` has a `tzinfo` that reports an offset.
#[inline]
pub fn offset_aware(value: &Bound<'_, PyAny>) -> PyResult<bool> {
    timestamps::offset_aware(value)
}
