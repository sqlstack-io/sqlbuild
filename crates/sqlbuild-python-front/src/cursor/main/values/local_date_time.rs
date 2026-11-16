//! The wall-clock fields of a `datetime`.

use crate::cursor::_helpers::values;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use sqlbuild_cursor::algebra::models::LocalDateTime;

/// The wall-clock fields of a `datetime`.
#[inline]
pub fn local_date_time(value: &Bound<'_, PyAny>) -> PyResult<LocalDateTime> {
    values::local_date_time(value)
}
