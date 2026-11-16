//! A Python value in orjson's supported domain, converted once at its data boundary.

use crate::compile_outputs::_helpers::json_report;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use sqlbuild_core::json::models::JsonValue;

/// A Python value in orjson's supported domain, converted once at its data boundary.
#[inline]
pub fn orjson_value(value: &Bound<'_, PyAny>) -> PyResult<JsonValue> {
    json_report::orjson_value(value)
}
