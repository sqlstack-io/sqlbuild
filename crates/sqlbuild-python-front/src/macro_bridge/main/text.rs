//! A Python `str` as JSON text; one holding a lone surrogate as UTF-16 code units.

use crate::macro_bridge::_helpers::store_keys;
use pyo3::prelude::Bound;
use pyo3::prelude::PyResult;
use pyo3::types::PyString;
use sqlbuild_core::json::models::JsonValue;

/// A Python `str` as JSON text; one holding a lone surrogate as UTF-16 code units.
#[inline]
pub fn text(value: &Bound<'_, PyString>) -> PyResult<JsonValue> {
    store_keys::text(value)
}
