//! Whether `value` is a `datetime`.

use crate::state_rows::_helpers::event_rows::timestamps;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Whether `value` is a `datetime`.
#[inline]
pub fn date_time_typed(value: &Bound<'_, PyAny>) -> PyResult<bool> {
    timestamps::date_time_typed(value)
}
