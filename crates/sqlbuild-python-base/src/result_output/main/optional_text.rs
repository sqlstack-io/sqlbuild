//! `value` when it is a string, else `null`.

use crate::result_output::_helpers::integration_values;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use sqlbuild_core::json::models::JsonValue;

/// `value` when it is a string, else `null`.
#[inline]
pub fn optional_text(value: &Bound<'_, PyAny>) -> PyResult<JsonValue> {
    integration_values::optional_text(value)
}
