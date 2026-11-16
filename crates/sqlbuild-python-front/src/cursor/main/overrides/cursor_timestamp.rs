//! A timestamp override as a `datetime`; dates start at midnight and text is ISO 8601.

use crate::cursor::_helpers::overrides::INTEGER_AS_TIMESTAMP;
use crate::cursor::_helpers::overrides::instance_of;
use crate::cursor::constants::DATETIME_MODULE;
use crate::cursor::main::conversion::algebra_error::algebra_error;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;

/// A timestamp override as a `datetime`; dates start at midnight and text is ISO 8601.
pub fn cursor_timestamp<'py>(value: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = value.py();
    if value.is_none() {
        return Ok(value.clone());
    }
    if instance_of(value, RecordId::TimestampValue)? {
        return native_field(value, "value");
    }
    let datetime: Bound<'py, PyAny> = imported(py, DATETIME_MODULE, "datetime")?;
    if instance_of(value, RecordId::DateValue)? {
        let midnight: Bound<'py, PyAny> = native_field(&datetime, "min")?.call_method0("time")?;
        return datetime.call_method1("combine", (native_field(value, "value")?, midnight));
    }
    if instance_of(value, RecordId::IntegerValue)? {
        return Err(algebra_error(py, INTEGER_AS_TIMESTAMP));
    }
    datetime.call_method1("fromisoformat", (value,))
}
