//! Whether `key` holds a string equal to `expected`, as Python's `==` compares them.

use crate::model_config::_helpers::config::typed_config::node;
use crate::model_config::_helpers::config::typed_config::type_error;
use crate::model_config::types::ConfigValues;
use crate::model_config::types::PyConfigNode;
use pyo3::prelude::{PyResult, Python};
use sqlbuild_model_config::config_values::main::checked_text::checked_text;
use sqlbuild_model_config::config_values::models::ConfigNode;
use std::sync::Arc;

/// Whether `key` holds a string equal to `expected`, as Python's `==` compares them.
pub fn config_text_is(
    py: Python<'_>,
    values: &ConfigValues<'_>,
    key: &str,
    expected: &str,
) -> PyResult<bool> {
    let node: Arc<PyConfigNode> = node(values, key)?;
    let text: Option<&PyConfigNode> =
        checked_text(key, Some(&node)).map_err(|error| type_error(py, &error))?;
    Ok(match text {
        Some(ConfigNode::Str(text)) => text == expected,
        Some(ConfigNode::Host(host)) if host.facts.strict_text => {
            host.facts.text.as_deref() == Some(expected)
        }
        _ => false,
    })
}
