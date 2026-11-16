//! Retained audit contracts; Python-authored factory results remain opaque values.

use crate::model_config::main::config::config_node::config_node;
use crate::model_config::main::config::config_object::config_object;
use pyo3::prelude::{PyResult, Python};
use sqlbuild_core::json::models::{JsonInteger, JsonValue};
use sqlbuild_discovery::model_files::main::config_value::config_value;
use sqlbuild_discovery::model_files::models::NativeAudit;
use sqlbuild_model_config::config_values::models::ConfigNode;
use sqlbuild_model_config::header_metadata::models::ParsedAudit;
use sqlbuild_sqltext::compiler::models::AuthoredValue;

use crate::model_config::types::HeaderValue;
use crate::model_config::types::PyConfigNode;
use crate::sqltext::main::authored_value_to_python::authored_value_to_python;
use sqlbuild_python_base::result_output::main::json_value::json_value;

/// Native discovery facts keep their authored values rather than building audit records.
pub(crate) fn authored_audit(
    py: Python<'_>,
    spec: &NativeAudit,
) -> PyResult<ParsedAudit<PyConfigNode>> {
    let value = |value: &AuthoredValue| match config_value(value) {
        Some(value) => Ok(value),
        None => config_node(authored_value_to_python(py, value)?.bind(py)),
    };
    Ok(ParsedAudit {
        definition_name: ConfigNode::Str(spec.definition_name.clone()),
        arguments: spec
            .arguments
            .iter()
            .map(|(key, item)| Ok((ConfigNode::Str(key.clone()), value(item)?)))
            .collect::<PyResult<_>>()?,
        name: spec.name.clone().map(ConfigNode::Str),
        description: spec.description.clone().map(ConfigNode::Str),
        severity: spec.severity.clone().map(ConfigNode::Str),
        run_scope: spec.run_scope.clone().map(ConfigNode::Str),
        always_run: spec.always_run.map(ConfigNode::Bool),
        thresholds: spec.thresholds,
        minimum_samples: spec.minimum_samples.as_ref().map(value).transpose()?,
        evidence_limit: spec.evidence_limit.as_ref().map(value).transpose()?,
        meta: spec.meta.as_ref().map(value).transpose()?,
    })
}

/// Preserve parsed leaves without rebuilding their public argument dictionaries.
pub(crate) fn retain_audit<N: HeaderValue>(
    py: Python<'_>,
    spec: &ParsedAudit<N>,
) -> PyResult<ParsedAudit<PyConfigNode>> {
    let optional = |node: &Option<N>| node.as_ref().map(|node| node.retained(py)).transpose();
    Ok(ParsedAudit {
        definition_name: spec.definition_name.retained(py)?,
        arguments: spec
            .arguments
            .iter()
            .map(|(key, value)| Ok((key.retained(py)?, value.retained(py)?)))
            .collect::<PyResult<_>>()?,
        name: optional(&spec.name)?,
        description: optional(&spec.description)?,
        severity: optional(&spec.severity)?,
        run_scope: optional(&spec.run_scope)?,
        always_run: optional(&spec.always_run)?,
        thresholds: spec.thresholds,
        minimum_samples: optional(&spec.minimum_samples)?,
        evidence_limit: optional(&spec.evidence_limit)?,
        meta: optional(&spec.meta)?,
    })
}

/// Read metadata without projecting retained containers; opaque user values keep Python semantics.
pub(crate) fn config_json_value(
    py: Python<'_>,
    value: &PyConfigNode,
) -> PyResult<Option<JsonValue>> {
    Ok(Some(match value {
        ConfigNode::Null => JsonValue::Null,
        ConfigNode::Bool(value) => JsonValue::Bool(*value),
        ConfigNode::Int(value) => JsonValue::Integer(JsonInteger::from(*value)),
        ConfigNode::Float(value) => JsonValue::Float(*value),
        ConfigNode::Str(value) => JsonValue::String(value.clone()),
        ConfigNode::List(items) | ConfigNode::Tuple(items) => {
            let mut result: Vec<JsonValue> = Vec::with_capacity(items.len());
            for item in items {
                let Some(value) = config_json_value(py, item)? else {
                    return Ok(None);
                };
                result.push(value);
            }
            JsonValue::Array(result)
        }
        ConfigNode::Map(entries) => {
            let mut result: Vec<(String, JsonValue)> = Vec::with_capacity(entries.len());
            for (key, item) in entries {
                let ConfigNode::Str(key) = key else {
                    return json_value(&config_object(py, value)?);
                };
                let Some(value) = config_json_value(py, item)? else {
                    return Ok(None);
                };
                result.push((key.clone(), value));
            }
            JsonValue::Object(result)
        }
        ConfigNode::Host(_) => return json_value(&config_object(py, value)?),
    }))
}
