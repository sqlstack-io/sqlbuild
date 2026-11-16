//! `key`'s boolean value, or `None` when absent or `None`.

use crate::model_config::_helpers::config::typed_config;
use crate::model_config::types::ConfigValues;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// `key`'s boolean value, or `None` when absent or `None`.
#[inline]
pub fn config_flag(py: Python<'_>, values: &ConfigValues<'_>, key: &str) -> PyResult<Option<bool>> {
    typed_config::config_flag(py, values, key)
}
