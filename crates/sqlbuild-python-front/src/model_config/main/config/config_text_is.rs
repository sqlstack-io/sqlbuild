//! Whether `key` holds a string equal to `expected`, as Python's `==` compares them.

use crate::model_config::_helpers::config::typed_config;
use crate::model_config::types::ConfigValues;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// Whether `key` holds a string equal to `expected`, as Python's `==` compares them.
#[inline]
pub fn config_text_is(
    py: Python<'_>,
    values: &ConfigValues<'_>,
    key: &str,
    expected: &str,
) -> PyResult<bool> {
    typed_config::config_text_is(py, values, key, expected)
}
