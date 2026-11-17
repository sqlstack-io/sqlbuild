//! One cursor value parsed from its boundary representation, keeping date-ness and offsets.

use crate::cursor::_helpers::scalars::INTEGER_CURSOR;
use crate::cursor::_helpers::scalars::TIMESTAMP_CURSOR;
use crate::cursor::_helpers::scalars::parse_integer;
use crate::cursor::_helpers::scalars::parse_timestamp_text;
use crate::cursor::_helpers::scalars::utc_normalized;
use crate::cursor::constants::DATETIME_MODULE;
use crate::cursor::main::conversion::algebra_error::algebra_error;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::PyString;
use pyo3::types::PyStringMethods;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::types::RecordId;

/// One cursor value parsed from its boundary representation, keeping date-ness and offsets.
pub fn parse_value<'py>(
    raw: &Bound<'py, PyAny>,
    cursor_type: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = raw.py();
    if cursor_type.eq(INTEGER_CURSOR)? {
        return parse_integer(raw);
    }
    if !cursor_type.eq(TIMESTAMP_CURSOR)? {
        return Err(algebra_error(
            py,
            &format!("unsupported cursor type: {}", cursor_type.str()?),
        ));
    }
    if raw.is_instance(&imported(py, DATETIME_MODULE, "datetime")?)? {
        let value: Bound<'py, PyAny> = utc_normalized(raw)?;
        return new_record_with(py, RecordId::TimestampValue, &[("value", value)]);
    }
    if raw.is_instance(&imported(py, DATETIME_MODULE, "date")?)? {
        return new_record_with(py, RecordId::DateValue, &[("value", raw.clone())]);
    }
    let Ok(text) = raw.downcast::<PyString>() else {
        return Err(algebra_error(
            py,
            &format!("invalid timestamp cursor value: {}", raw.str()?),
        ));
    };
    parse_timestamp_text(raw, text.to_str()?)
}
