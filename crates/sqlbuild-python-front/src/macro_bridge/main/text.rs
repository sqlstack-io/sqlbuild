//! A Python `str` as JSON text; one holding a lone surrogate as UTF-16 code units.

use pyo3::prelude::{Bound, PyResult};
use pyo3::types::{PyString, PyStringMethods};
use sqlbuild_core::json::models::JsonValue;
use sqlbuild_python_base::boundary::main::utf16_units::utf16_units;

/// A Python `str` as JSON text; one holding a lone surrogate as UTF-16 code units.
pub fn text(value: &Bound<'_, PyString>) -> PyResult<JsonValue> {
    match value.to_str() {
        Ok(text) => Ok(JsonValue::String(text.to_owned())),
        Err(_) => utf16_units(value).map(JsonValue::Utf16),
    }
}
