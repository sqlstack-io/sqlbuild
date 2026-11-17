//! The wall-clock fields of a `datetime`.

use crate::cursor::_helpers::values::civil_date;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_cursor::algebra::models::LocalDateTime;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// The wall-clock fields of a `datetime`.
pub fn local_date_time(value: &Bound<'_, PyAny>) -> PyResult<LocalDateTime> {
    Ok(LocalDateTime {
        date: civil_date(value)?,
        hour: native_field(value, "hour")?.extract()?,
        minute: native_field(value, "minute")?.extract()?,
        second: native_field(value, "second")?.extract()?,
        microsecond: native_field(value, "microsecond")?.extract()?,
    })
}
