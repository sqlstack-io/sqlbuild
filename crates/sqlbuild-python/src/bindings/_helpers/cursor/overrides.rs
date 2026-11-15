//! Executor cursor bounds from `--start-cursor`/`--end-cursor` overrides.

use pyo3::IntoPyObject;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyInt, PyString, PyStringMethods};
use sqlbuild_cursor::algebra::main::parse_integer_text::parse_integer_text;

use crate::bindings::_helpers::cursor::conversion::algebra_error;
use crate::bindings::_helpers::cursor::values::DATETIME_MODULE;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::records::main::construct::record_class::record_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;

const DECIMAL_MODULE: &str = "decimal";
const INTEGER_AS_TIMESTAMP: &str = "an integer cursor cannot be used as a timestamp";
const TIMESTAMP_AS_INTEGER: &str = "a timestamp cursor cannot be used as an integer";

fn instance_of(value: &Bound<'_, PyAny>, class: RecordId) -> PyResult<bool> {
    value.is_instance(&record_class(value.py(), class)?)
}

/// A timestamp override as a `datetime`; dates start at midnight and text is ISO 8601.
pub(crate) fn cursor_timestamp<'py>(value: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
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

/// An integer override as an `int`; text is read the way `int(Decimal(text))` reads it.
pub(crate) fn cursor_integer<'py>(value: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = value.py();
    if value.is_none() {
        return Ok(value.clone());
    }
    if instance_of(value, RecordId::IntegerValue)? {
        return native_field(value, "value");
    }
    if instance_of(value, RecordId::DateValue)? || instance_of(value, RecordId::TimestampValue)? {
        return Err(algebra_error(py, TIMESTAMP_AS_INTEGER));
    }
    if let Ok(text) = value.downcast::<PyString>()
        && let Some(parsed) = parse_integer_text(text.to_str()?)
    {
        return Ok(parsed.into_pyobject(py)?.into_any());
    }
    let decimal: Bound<'py, PyAny> = imported(py, DECIMAL_MODULE, "Decimal")?.call1((value,))?;
    py.get_type::<PyInt>().call1((decimal,))
}
