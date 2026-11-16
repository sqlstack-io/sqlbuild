//! A JSON string.

use crate::result_output::_helpers::integration_values;
use sqlbuild_core::json::models::JsonValue;

/// A JSON string.
#[inline]
pub fn text(value: &str) -> JsonValue {
    integration_values::text(value)
}
