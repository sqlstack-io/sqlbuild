//! A plain JSON Python value, or the contract failure of a non-JSON value.

use crate::result_output::_helpers::integration_values;
use crate::result_output::types::Checked;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use sqlbuild_core::json::models::JsonValue;

/// A plain JSON Python value, or the contract failure of a non-JSON value.
#[inline]
pub fn required_json(value: &Bound<'_, PyAny>) -> Checked<JsonValue> {
    integration_values::required_json(value)
}
