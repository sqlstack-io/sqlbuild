//! `orjson.dumps(value, option=OPT_SORT_KEYS)` for an already-native payload.

use crate::compile_outputs::_helpers::json_report;
use pyo3::prelude::PyResult;
use sqlbuild_core::json::models::JsonValue;

/// `orjson.dumps(value, option=OPT_SORT_KEYS)` for an already-native payload.
#[inline]
pub fn orjson_sorted(value: &JsonValue) -> PyResult<String> {
    json_report::orjson_sorted(value)
}
