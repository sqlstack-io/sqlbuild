//! `_bounded_metadata`: the value when it encodes within the metadata bounds, else `{}`.

use crate::result_output::_helpers::integration_values;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use sqlbuild_core::json::models::JsonValue;

/// `_bounded_metadata`: the value when it encodes within the metadata bounds, else `{}`.
#[inline]
pub fn bounded_metadata(value: &Bound<'_, PyAny>) -> PyResult<JsonValue> {
    integration_values::bounded_metadata(value)
}
