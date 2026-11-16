//! A plain JSON Python value, or `None` when it holds something JSON cannot carry.

use crate::result_output::_helpers::integration_values;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use sqlbuild_core::json::models::JsonValue;

/// A plain JSON Python value, or `None` when it holds something JSON cannot carry.
#[inline]
pub fn json_value(value: &Bound<'_, PyAny>) -> PyResult<Option<JsonValue>> {
    integration_values::json_value(value)
}
