//! `key`'s string value, or `None` when absent or `None`.

use crate::model_config::_helpers::config::typed_config;
use crate::model_config::types::ConfigValues;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// `key`'s string value, or `None` when absent or `None`.
#[inline]
pub fn config_text(
    py: Python<'_>,
    values: &ConfigValues<'_>,
    key: &str,
) -> PyResult<Option<String>> {
    typed_config::config_text(py, values, key)
}
