//! Python `datetime` values normalized to UTC wall time.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_cursor::algebra::models::LocalDateTime;

use crate::bindings::_helpers::cursor::values::{DATETIME_MODULE, local_date_time};
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// Whether `value` is a `datetime`.
pub(crate) fn date_time_typed(value: &Bound<'_, PyAny>) -> PyResult<bool> {
    value.is_instance(&imported(value.py(), DATETIME_MODULE, "datetime")?)
}

/// Whether `value` has a `tzinfo` that reports an offset.
pub(crate) fn offset_aware(value: &Bound<'_, PyAny>) -> PyResult<bool> {
    Ok(!native_field(value, "tzinfo")?.is_none() && !value.call_method0("utcoffset")?.is_none())
}

/// `value.astimezone(UTC)`.
pub(crate) fn in_utc<'py>(value: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    value.call_method1(
        "astimezone",
        (imported(value.py(), DATETIME_MODULE, "UTC")?,),
    )
}

/// The UTC wall time of `value`, reading a naive or offset-less value as UTC.
pub(crate) fn presumed_utc(value: &Bound<'_, PyAny>) -> PyResult<LocalDateTime> {
    if offset_aware(value)? {
        return local_date_time(&in_utc(value)?);
    }
    local_date_time(value)
}
