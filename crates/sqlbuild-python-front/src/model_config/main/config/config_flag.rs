//! `key`'s boolean value, or `None` when absent or `None`.

use crate::model_config::_helpers::config::typed_config::node;
use crate::model_config::_helpers::config::typed_config::type_error;
use crate::model_config::types::ConfigValues;
use crate::model_config::types::PyConfigNode;
use pyo3::prelude::{PyResult, Python};
use sqlbuild_model_config::config_values::main::checked_flag::checked_flag;
use std::sync::Arc;

/// `key`'s boolean value, or `None` when absent or `None`.
pub fn config_flag(py: Python<'_>, values: &ConfigValues<'_>, key: &str) -> PyResult<Option<bool>> {
    let node: Arc<PyConfigNode> = node(values, key)?;
    checked_flag(key, Some(&node)).map_err(|error| type_error(py, &error))
}
