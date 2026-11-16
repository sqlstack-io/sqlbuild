//! orjson's value for `value`: enums by value, dataclasses by public attribute, dates in ISO form.

use crate::compiled_project::_helpers::values;
use pyo3::prelude::{Bound, PyAny, PyResult};
use serde_json::Value;

/// orjson's value for `value`: enums by value, dataclasses by public attribute, dates in ISO form.
#[inline]
pub fn plain_value(value: &Bound<'_, PyAny>) -> PyResult<Value> {
    values::plain_value(value)
}
