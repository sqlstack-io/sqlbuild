//! `key`'s string value, or `None` when absent or `None`.

use crate::model_config::_helpers::config::typed_config::node;
use crate::model_config::_helpers::config::typed_config::node_text;
use crate::model_config::_helpers::config::typed_config::type_error;
use crate::model_config::types::ConfigValues;
use crate::model_config::types::PyConfigNode;
use pyo3::prelude::{PyResult, Python};
use sqlbuild_model_config::config_values::main::checked_text::checked_text;
use std::sync::Arc;

/// `key`'s string value, or `None` when absent or `None`.
pub fn config_text(
    py: Python<'_>,
    values: &ConfigValues<'_>,
    key: &str,
) -> PyResult<Option<String>> {
    let node: Arc<PyConfigNode> = node(values, key)?;
    checked_text(key, Some(&node))
        .map_err(|error| type_error(py, &error))?
        .map(|value| node_text(py, value))
        .transpose()
}
