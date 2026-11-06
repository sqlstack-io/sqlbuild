//! Authored header values retained as config nodes without reading their Python projection.

use sqlbuild_model_config::config_values::models::ConfigNode;
use sqlbuild_sqltext::compiler::main::header_word_value::header_word_value;
use sqlbuild_sqltext::compiler::models::{AuthoredValue, HeaderWord};

use crate::model_files::_helpers::native_nodes::dict_entries;

/// Retain builtin values natively; hook objects, declaration markers and huge integers need host identity.
pub fn config_value<H>(value: &AuthoredValue) -> Option<ConfigNode<H>> {
    Some(match value {
        AuthoredValue::Null => ConfigNode::Null,
        AuthoredValue::Boolean(value) => ConfigNode::Bool(*value),
        AuthoredValue::String(value) => ConfigNode::Str(value.clone()),
        AuthoredValue::BareWord(value) => match header_word_value(value) {
            HeaderWord::Bool(value) => ConfigNode::Bool(value),
            HeaderWord::Null => ConfigNode::Null,
            HeaderWord::Integer(value) => match value.parse() {
                Ok(value) => ConfigNode::Int(value),
                Err(_) => return None,
            },
            HeaderWord::Float(value) => ConfigNode::Float(value),
            HeaderWord::Text => ConfigNode::Str(value.clone()),
        },
        AuthoredValue::List(values) => {
            ConfigNode::List(values.iter().map(config_value).collect::<Option<_>>()?)
        }
        AuthoredValue::Tuple(values) => {
            ConfigNode::Tuple(values.iter().map(config_value).collect::<Option<_>>()?)
        }
        AuthoredValue::Map(values) => ConfigNode::Map(
            dict_entries(values)
                .into_iter()
                .map(|(key, value)| Some((ConfigNode::Str(key.to_owned()), config_value(value)?)))
                .collect::<Option<_>>()?,
        ),
        AuthoredValue::Set(_)
        | AuthoredValue::TypedConstant(_)
        | AuthoredValue::InlineSqlHook(_)
        | AuthoredValue::NamedSqlHook(_, _)
        | AuthoredValue::PythonHook(_, _) => return None,
    })
}
