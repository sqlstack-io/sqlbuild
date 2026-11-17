//! The UTC wall time of `value`, reading a naive or offset-less value as UTC.

use crate::cursor::main::values::local_date_time::local_date_time;
use crate::state_rows::main::event_rows::in_utc::in_utc;
use crate::state_rows::main::event_rows::offset_aware::offset_aware;
use pyo3::prelude::{Bound, PyAny, PyResult};
use sqlbuild_cursor::algebra::models::LocalDateTime;

/// The UTC wall time of `value`, reading a naive or offset-less value as UTC.
pub fn presumed_utc(value: &Bound<'_, PyAny>) -> PyResult<LocalDateTime> {
    if offset_aware(value)? {
        return local_date_time(&in_utc(value)?);
    }
    local_date_time(value)
}
