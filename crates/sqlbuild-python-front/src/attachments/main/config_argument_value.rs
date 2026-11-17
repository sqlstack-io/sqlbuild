//! Native-built argument containers need no intermediate Python list or tuple.

use crate::attachments::_helpers::argument_values::argument_value;
use crate::model_config::main::config::config_object::config_object;
use crate::model_config::types::PyConfigNode;
use pyo3::prelude::{PyResult, Python};
use sqlbuild_attachments::audits::models::ArgumentValue;
use sqlbuild_model_config::config_values::models::ConfigNode;

/// Native-built argument containers need no intermediate Python list or tuple.
pub fn config_argument_value(py: Python<'_>, value: &PyConfigNode) -> PyResult<ArgumentValue> {
    Ok(match value {
        ConfigNode::Null => ArgumentValue::Null,
        ConfigNode::Bool(value) => ArgumentValue::Boolean(*value),
        ConfigNode::Int(value) => ArgumentValue::Number(value.to_string()),
        ConfigNode::Str(value) => ArgumentValue::Text(value.clone()),
        ConfigNode::List(items) => ArgumentValue::List(
            items
                .iter()
                .map(|item| config_argument_value(py, item))
                .collect::<PyResult<_>>()?,
        ),
        ConfigNode::Tuple(items) => ArgumentValue::Tuple(
            items
                .iter()
                .map(|item| config_argument_value(py, item))
                .collect::<PyResult<_>>()?,
        ),
        ConfigNode::Map(_) => ArgumentValue::Opaque,
        ConfigNode::Float(_) | ConfigNode::Host(_) => {
            return argument_value(&config_object(py, value)?);
        }
    })
}
