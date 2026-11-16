//! `str(value or "")`.

use crate::result_output::_helpers::integration_values;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use sqlbuild_core::json::models::JsonValue;

/// `str(value or "")`.
#[inline]
pub fn text_or_empty(value: &Bound<'_, PyAny>) -> PyResult<JsonValue> {
    integration_values::text_or_empty(value)
}
