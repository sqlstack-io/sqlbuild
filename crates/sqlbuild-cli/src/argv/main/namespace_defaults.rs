//! The namespace a parse without an interpreter starts from.

use crate::argv::main::cli_namespace_defaults::cli_namespace_defaults;
use crate::argv::models::{NamespaceDefault, Value};

/// The parse's starting namespace: every `CliNamespace` default.
pub(crate) fn namespace_defaults() -> Vec<(String, Value)> {
    cli_namespace_defaults()
        .iter()
        .map(|(name, default)| {
            let value: Value = match default {
                NamespaceDefault::None => Value::None,
                NamespaceDefault::Bool(flag) => Value::Bool(*flag),
                NamespaceDefault::Int(number) => Value::Int(number.to_string()),
                NamespaceDefault::Text(text) => Value::Str((*text).to_owned()),
                NamespaceDefault::EmptyList => Value::List(Vec::new()),
                NamespaceDefault::EmptyDict => Value::EmptyDict,
                NamespaceDefault::CompileLineageFast | NamespaceDefault::ColumnLineageRich => {
                    Value::Host(0)
                }
            };
            ((*name).to_owned(), value)
        })
        .collect()
}
