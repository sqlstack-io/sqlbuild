//! The UTC wall time of `value`, reading a naive or offset-less value as UTC.

use crate::state_rows::_helpers::event_rows::timestamps;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use sqlbuild_cursor::algebra::models::LocalDateTime;

/// The UTC wall time of `value`, reading a naive or offset-less value as UTC.
#[inline]
pub fn presumed_utc(value: &Bound<'_, PyAny>) -> PyResult<LocalDateTime> {
    timestamps::presumed_utc(value)
}
