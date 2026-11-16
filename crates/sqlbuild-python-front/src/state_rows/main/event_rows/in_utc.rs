//! `value.astimezone(UTC)`.

use crate::state_rows::_helpers::event_rows::timestamps;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// `value.astimezone(UTC)`.
#[inline]
pub fn in_utc<'py>(value: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    timestamps::in_utc(value)
}
