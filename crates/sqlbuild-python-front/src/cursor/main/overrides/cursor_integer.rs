//! An integer override as an `int`; text is read the way `int(Decimal(text))` reads it.

use crate::cursor::_helpers::overrides::DECIMAL_MODULE;
use crate::cursor::_helpers::overrides::TIMESTAMP_AS_INTEGER;
use crate::cursor::_helpers::overrides::instance_of;
use crate::cursor::main::conversion::algebra_error::algebra_error;
use pyo3::IntoPyObject;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyInt, PyString, PyStringMethods};
use sqlbuild_cursor::algebra::main::parse_integer_text::parse_integer_text;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;

/// An integer override as an `int`; text is read the way `int(Decimal(text))` reads it.
pub fn cursor_integer<'py>(value: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
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
    if let Ok(text) = value.cast::<PyString>()
        && let Some(parsed) = parse_integer_text(text.to_str()?)
    {
        return Ok(parsed.into_pyobject(py)?.into_any());
    }
    let decimal: Bound<'py, PyAny> = imported(py, DECIMAL_MODULE, "Decimal")?.call1((value,))?;
    py.get_type::<PyInt>().call1((decimal,))
}
